#include "SchemaParser.hpp"

#include <cerrno>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/FileReader.hpp>
#include <log_surgeon/finite_automata/Capture.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Lalr1Parser.hpp>
#include <log_surgeon/parser_types.hpp>
#include <log_surgeon/Reader.hpp>
#include <log_surgeon/utils.hpp>

#include "log_surgeon/ParserAst.hpp"

using ParserValueRegex = log_surgeon::ParserValue<std::unique_ptr<
        log_surgeon::finite_automata::RegexAST<log_surgeon::finite_automata::ByteNfaState>
>>;
using RegexASTByte
        = log_surgeon::finite_automata::RegexAST<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTGroupByte
        = log_surgeon::finite_automata::RegexASTGroup<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTIntegerByte
        = log_surgeon::finite_automata::RegexASTInteger<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTLiteralByte
        = log_surgeon::finite_automata::RegexASTLiteral<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTMultiplicationByte = log_surgeon::finite_automata::
        RegexASTMultiplication<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTOrByte
        = log_surgeon::finite_automata::RegexASTOr<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTCatByte
        = log_surgeon::finite_automata::RegexASTCat<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTCaptureByte
        = log_surgeon::finite_automata::RegexASTCapture<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTEmptyByte
        = log_surgeon::finite_automata::RegexASTEmpty<log_surgeon::finite_automata::ByteNfaState>;

using std::make_unique;
using std::string;
using std::string_view;
using std::unique_ptr;

namespace log_surgeon {
SchemaParser::SchemaParser() {
    add_lexical_rules();
    add_productions();
    generate();
}

auto SchemaParser::generate_schema_ast(Reader& reader) -> unique_ptr<SchemaAST> {
    NonTerminal nonterminal = parse(reader);
    unique_ptr<SchemaAST> schema_ast(
            dynamic_cast<SchemaAST*>(nonterminal.release_parser_ast().release())
    );
    return schema_ast;
}

auto SchemaParser::try_schema_file(string const& schema_file_path) -> unique_ptr<SchemaAST> {
    FileReader schema_reader;
    ErrorCode error_code = schema_reader.try_open(schema_file_path);
    if (ErrorCode::Success != error_code) {
        if (ErrorCode::Errno == error_code) {
            throw std::runtime_error(
                    strfmt("Failed to read '%s', errno=%d", schema_file_path.c_str(), errno)
            );
        }
        auto const code{static_cast<std::underlying_type_t<ErrorCode>>(error_code)};
        throw std::runtime_error(
                strfmt("Failed to read '%s', error_code=%d", schema_file_path.c_str(), code)
        );
    }
    SchemaParser sp;
    Reader reader{[&](char* buf, size_t count, size_t& read_to) -> ErrorCode {
        schema_reader.read(buf, count, read_to);
        if (read_to == 0) {
            return ErrorCode::EndOfFile;
        }
        return ErrorCode::Success;
    }};
    unique_ptr<SchemaAST> schema_ast = sp.generate_schema_ast(reader);
    schema_reader.close();
    return schema_ast;
}

auto SchemaParser::try_schema_string(string_view const schema_string) -> unique_ptr<SchemaAST> {
    Reader reader{[&](char* dst_buf, size_t count, size_t& read_to) -> ErrorCode {
        uint32_t unparsed_string_pos = 0;
        std::span<char> const buf{dst_buf, count};
        if (unparsed_string_pos + count > schema_string.length()) {
            count = schema_string.length() - unparsed_string_pos;
        }
        read_to = count;
        if (read_to == 0) {
            return ErrorCode::EndOfFile;
        }
        for (uint32_t i = 0; i < count; i++) {
            buf[i] = schema_string[unparsed_string_pos + i];
        }
        unparsed_string_pos += count;
        return ErrorCode::Success;
    }};
    SchemaParser sp;
    return sp.generate_schema_ast(reader);
}

static auto new_identifier_rule(NonTerminal* m) -> unique_ptr<IdentifierAST> {
    return make_unique<IdentifierAST>(IdentifierAST{m->token_cast(0).to_string().at(0)});
}

static auto existing_identifier_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto parser_ast{m->non_terminal_cast(0).release_parser_ast()};
    auto* identifier_ast{dynamic_cast<IdentifierAST*>(parser_ast.get())};
    identifier_ast->add_character(m->token_cast(1).to_string().at(0));
    return parser_ast;
}

static auto schema_var_rule(NonTerminal* m) -> unique_ptr<SchemaVarAST> {
    auto identifier_ast{dynamic_cast<IdentifierAST&>(m->non_terminal_cast(1).get_parser_ast())};
    return make_unique<SchemaVarAST>(
            identifier_ast.m_name,
            std::move(m->non_terminal_cast(3).get_parser_ast().get<unique_ptr<RegexASTByte>>()),
            m->token_cast(2).get_line_num()
    );
}

static auto new_schema_rule(NonTerminal* /* m */) -> unique_ptr<SchemaAST> {
    return make_unique<SchemaAST>();
}

static auto new_schema_rule_with_var(NonTerminal* m) -> unique_ptr<SchemaAST> {
    auto schema_ast{make_unique<SchemaAST>()};
    schema_ast->add_schema_var(m->non_terminal_cast(0).release_parser_ast());
    return schema_ast;
}

static auto new_schema_rule_with_delimiters(NonTerminal* m) -> unique_ptr<SchemaAST> {
    auto schema_ast{make_unique<SchemaAST>()};
    schema_ast->add_delimiters(m->non_terminal_cast(2).release_parser_ast());
    return schema_ast;
}

static auto existing_schema_rule_with_delimiter(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<SchemaAST> schema_ast(
            dynamic_cast<SchemaAST*>(m->non_terminal_cast(0).release_parser_ast().release())
    );
    schema_ast->add_delimiters(m->non_terminal_cast(4).release_parser_ast());
    return schema_ast;
}

auto SchemaParser::existing_schema_rule(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<SchemaAST> schema_ast{
            dynamic_cast<SchemaAST*>(m->non_terminal_cast(0).release_parser_ast().release())
    };
    schema_ast->add_schema_var(m->non_terminal_cast(2).release_parser_ast());
    return schema_ast;
}

static auto regex_capture_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto identifier_ast{dynamic_cast<IdentifierAST&>(m->non_terminal_cast(3).get_parser_ast())};
    return make_unique<ParserValueRegex>(make_unique<RegexASTCaptureByte>(
            std::move(m->non_terminal_cast(5).get_parser_ast().get<unique_ptr<RegexASTByte>>()),
            make_unique<finite_automata::Capture>(identifier_ast.m_name)
    ));
}

static auto identity_rule_ParserASTSchema(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<SchemaAST> schema_ast{
            dynamic_cast<SchemaAST*>(m->non_terminal_cast(0).release_parser_ast().release())
    };
    return schema_ast;
}

static auto regex_identity_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            std::move(m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>())
    );
}

static auto regex_cat_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTCatByte>(
            std::move(m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>()),
            std::move(m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>())
    ));
}

static auto regex_or_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTOrByte>(
            std::move(m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>()),
            std::move(m->non_terminal_cast(2).get_parser_ast().get<unique_ptr<RegexASTByte>>())
    ));
}

static auto regex_match_zero_or_more_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    // To handle negative captures we treat `R*` as `R+ | ∅`.
    return make_unique<ParserValueRegex>(make_unique<RegexASTOrByte>(
            make_unique<RegexASTEmptyByte>(),
            make_unique<RegexASTMultiplicationByte>(
                    std::move(
                            m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>()
                    ),
                    1,
                    0
            )
    ));
}

static auto regex_match_one_or_more_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTMultiplicationByte>(
            std::move(m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>()),
            1,
            0
    ));
}

static auto regex_match_exactly_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto* int_ast{dynamic_cast<RegexASTIntegerByte*>(
            m->non_terminal_cast(2).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
    )};
    uint32_t reps{0};
    auto num_digits{int_ast->get_digits().size()};
    for (size_t i{0}; i < num_digits; ++i) {
        reps += int_ast->get_digit(i) * static_cast<uint32_t>(pow(10, num_digits - i - 1));
    }
    return make_unique<ParserValueRegex>(make_unique<RegexASTMultiplicationByte>(
            std::move(m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>()),
            reps,
            reps
    ));
}

static auto regex_match_range_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto* int_ast{dynamic_cast<RegexASTIntegerByte*>(
            m->non_terminal_cast(2).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
    )};
    uint32_t min{0};
    auto num_digits{int_ast->get_digits().size()};
    for (size_t i{0}; i < num_digits; ++i) {
        min += int_ast->get_digit(i) * static_cast<uint32_t>(pow(10, num_digits - i - 1));
    }

    int_ast = dynamic_cast<RegexASTIntegerByte*>(
            m->non_terminal_cast(4).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
    );
    uint32_t max{0};
    num_digits = int_ast->get_digits().size();
    for (uint32_t i{0}; i < num_digits; ++i) {
        max += int_ast->get_digit(i) * static_cast<uint32_t>(pow(10, num_digits - i - 1));
    }

    auto& regex_ast{m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>()};
    if (0 == min) {
        // To handle negative captures we treat `R*` as `R+ | ∅`.
        return make_unique<ParserValueRegex>(make_unique<RegexASTOrByte>(
                make_unique<RegexASTEmptyByte>(),
                make_unique<RegexASTMultiplicationByte>(std::move(regex_ast), 1, max)
        ));
    }
    return make_unique<ParserValueRegex>(
            make_unique<RegexASTMultiplicationByte>(std::move(regex_ast), min, max)
    );
}

static auto regex_add_literal_existing_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTGroupByte>(
            dynamic_cast<RegexASTGroupByte*>(
                    m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            ),
            dynamic_cast<RegexASTLiteralByte*>(
                    m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            )
    ));
}

static auto regex_add_range_existing_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTGroupByte>(
            dynamic_cast<RegexASTGroupByte*>(
                    m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            ),
            dynamic_cast<RegexASTGroupByte*>(
                    m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            )
    ));
}

static auto regex_add_literal_new_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            make_unique<RegexASTGroupByte>(dynamic_cast<RegexASTLiteralByte*>(
                    m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            ))
    );
}

static auto regex_add_range_new_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            make_unique<RegexASTGroupByte>(dynamic_cast<RegexASTGroupByte*>(
                    m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            ))
    );
}

static auto regex_complement_incomplete_group_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTGroupByte>());
}

static auto regex_range_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTGroupByte>(
            dynamic_cast<RegexASTLiteralByte*>(
                    m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            ),
            dynamic_cast<RegexASTLiteralByte*>(
                    m->non_terminal_cast(2).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            )
    ));
}

static auto regex_middle_identity_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            std::move(m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>())
    );
}

static auto regex_literal_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            make_unique<RegexASTLiteralByte>(m->token_cast(0).to_string().at(0))
    );
}

static auto regex_cancel_literal_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            make_unique<RegexASTLiteralByte>(m->token_cast(1).to_string().at(0))
    );
}

static auto regex_existing_integer_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTIntegerByte>(
            dynamic_cast<RegexASTIntegerByte*>(
                    m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
            ),
            m->token_cast(1).to_string().at(0)
    ));
}

static auto regex_new_integer_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(
            make_unique<RegexASTIntegerByte>(m->token_cast(0).to_string().at(0))
    );
}

static auto regex_digit_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTGroupByte>('0', '9'));
}

static auto regex_wildcard_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    auto regex_wildcard{make_unique<RegexASTGroupByte>(0, cUnicodeMax)};
    regex_wildcard->set_is_wildcard_true();
    return make_unique<ParserValueRegex>(std::move(regex_wildcard));
}

static auto regex_vertical_tab_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTLiteralByte>('\v'));
}

static auto regex_form_feed_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTLiteralByte>('\f'));
}

static auto regex_tab_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTLiteralByte>('\t'));
}

static auto regex_char_return_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTLiteralByte>('\r'));
}

static auto regex_newline_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return make_unique<ParserValueRegex>(make_unique<RegexASTLiteralByte>('\n'));
}

static auto regex_white_space_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    auto regex_ast_group{
            make_unique<RegexASTGroupByte>(RegexASTGroupByte({' ', '\t', '\r', '\n', '\v', '\f'}))
    };
    return make_unique<ParserValueRegex>(std::move(regex_ast_group));
}

static auto existing_delimiter_string_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto parser_ast{m->non_terminal_cast(0).release_parser_ast()};
    auto* delimiter_ast{dynamic_cast<DelimiterStringAST*>(parser_ast.get())};
    auto* byte_ast{dynamic_cast<RegexASTLiteralByte*>(
            m->non_terminal_cast(1).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
    )};
    delimiter_ast->add_delimiter(byte_ast->get_character());
    return parser_ast;
}

static auto new_delimiter_string_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto* byte_ast{dynamic_cast<RegexASTLiteralByte*>(
            m->non_terminal_cast(0).get_parser_ast().get<unique_ptr<RegexASTByte>>().get()
    )};
    return make_unique<DelimiterStringAST>(byte_ast->get_character());
}

auto SchemaParser::add_lexical_rules() -> void {
    if (m_special_regex_characters.empty()) {
        m_special_regex_characters.emplace('(', "Lparen");
        m_special_regex_characters.emplace(')', "Rparen");
        m_special_regex_characters.emplace('*', "Star");
        m_special_regex_characters.emplace('+', "Plus");
        m_special_regex_characters.emplace('-', "Dash");
        m_special_regex_characters.emplace('.', "Dot");
        m_special_regex_characters.emplace('[', "Lbracket");
        m_special_regex_characters.emplace(']', "Rbracket");
        m_special_regex_characters.emplace('\\', "Backslash");
        m_special_regex_characters.emplace('^', "Hat");
        m_special_regex_characters.emplace('{', "Lbrace");
        m_special_regex_characters.emplace('}', "Rbrace");
        m_special_regex_characters.emplace('|', "Vbar");
        m_special_regex_characters.emplace('<', "Langle");
        m_special_regex_characters.emplace('>', "Rangle");
        m_special_regex_characters.emplace('?', "QuestionMark");
    }
    for (auto const& [special_regex_char, special_regex_name] : m_special_regex_characters) {
        add_token(special_regex_name, special_regex_char);
    }
    add_token("Tab", '\t');  // 9
    add_token("NewLine", '\n');  // 10
    add_token("VerticalTab", '\v');  // 11
    add_token("FormFeed", '\f');  // 12
    add_token("CarriageReturn", '\r');  // 13
    add_token("Space", ' ');
    add_token("Bang", '!');
    add_token("Quotation", '"');
    add_token("Hash", '#');
    add_token("DollarSign", '$');
    add_token("Percent", '%');
    add_token("Ampersand", '&');
    add_token("Apostrophe", '\'');
    add_token("Comma", ',');
    add_token("ForwardSlash", '/');
    add_token_group("Numeric", make_unique<RegexASTGroupByte>('0', '9'));
    add_token("Colon", ':');
    add_token("SemiColon", ';');
    add_token("Equal", '=');
    add_token("At", '@');
    add_token_group("AlphaNumeric", make_unique<RegexASTGroupByte>('a', 'z'));
    add_token_group("AlphaNumeric", make_unique<RegexASTGroupByte>('A', 'Z'));
    add_token_group("AlphaNumeric", make_unique<RegexASTGroupByte>('0', '9'));
    add_token("Underscore", '_');
    add_token("Backtick", '`');
    add_token("Tilde", '~');
    add_token("d", 'd');
    add_token("s", 's');
    add_token("n", 'n');
    add_token("r", 'r');
    add_token("t", 't');
    add_token("f", 'f');
    add_token("v", 'v');
    add_token_chain("Delimiters", "delimiters");
    // RegexASTGroupByte default constructs to an m_negate group, so we add the only two characters
    // which can't be in a comment, the newline and carriage return characters as they signify the
    // end of the comment.
    unique_ptr<RegexASTGroupByte> comment_characters = make_unique<RegexASTGroupByte>();
    comment_characters->add_literal('\r');
    comment_characters->add_literal('\n');
    add_token_group("CommentCharacters", std::move(comment_characters));
    add_token_group("IdentifierCharacters", make_unique<RegexASTGroupByte>('a', 'z'));
    add_token_group("IdentifierCharacters", make_unique<RegexASTGroupByte>('A', 'Z'));
    add_token_group("IdentifierCharacters", make_unique<RegexASTGroupByte>('0', '9'));
    add_token("IdentifierCharacters", '_');
}

auto SchemaParser::add_productions() -> void {
    // add_production("Schema", {}, new_schema_rule);
    add_production("Schema", {"Comment"}, new_schema_rule);
    add_production("Schema", {"SchemaVar"}, new_schema_rule_with_var);
    add_production(
            "Schema",
            {"Delimiters", "Colon", "DelimiterString"},
            new_schema_rule_with_delimiters
    );
    add_production("Schema", {"Schema", "PortableNewLine"}, identity_rule_ParserASTSchema);
    add_production(
            "Schema",
            {"Schema", "PortableNewLine", "Comment"},
            identity_rule_ParserASTSchema
    );
    add_production(
            "Schema",
            {"Schema", "PortableNewLine", "SchemaVar"},
            std::bind(&SchemaParser::existing_schema_rule, this, std::placeholders::_1)
    );
    add_production(
            "Schema",
            {"Schema", "PortableNewLine", "Delimiters", "Colon", "DelimiterString"},
            existing_schema_rule_with_delimiter
    );
    add_production(
            "DelimiterString",
            {"DelimiterString", "Literal"},
            existing_delimiter_string_rule
    );
    add_production("DelimiterString", {"Literal"}, new_delimiter_string_rule);
    add_production("PortableNewLine", {"CarriageReturn", "NewLine"}, nullptr);
    add_production("PortableNewLine", {"NewLine"}, nullptr);
    add_production("Comment", {"ForwardSlash", "ForwardSlash", "Text"}, nullptr);
    add_production("Text", {"Text", "CommentCharacters"}, nullptr);
    add_production("Text", {"CommentCharacters"}, nullptr);
    add_production("Text", {"Text", "Delimiters"}, nullptr);
    add_production("Text", {"Delimiters"}, nullptr);
    add_production(
            "SchemaVar",
            {"WhitespaceStar", "Identifier", "Colon", "Regex"},
            schema_var_rule
    );
    add_production("Identifier", {"Identifier", "IdentifierCharacters"}, existing_identifier_rule);
    add_production("Identifier", {"IdentifierCharacters"}, new_identifier_rule);
    add_production("WhitespaceStar", {"WhitespaceStar", "Space"}, nullptr);
    add_production("WhitespaceStar", {}, nullptr);
    add_production("Regex", {"Concat"}, regex_identity_rule);
    add_production("Concat", {"Concat", "Or"}, regex_cat_rule);
    add_production("Concat", {"Or"}, regex_identity_rule);
    add_production("Or", {"Or", "Vbar", "Literal"}, regex_or_rule);
    add_production("Or", {"MatchStar"}, regex_identity_rule);
    add_production("Or", {"MatchPlus"}, regex_identity_rule);
    add_production("Or", {"MatchExact"}, regex_identity_rule);
    add_production("Or", {"MatchRange"}, regex_identity_rule);
    add_production("Or", {"CompleteGroup"}, regex_identity_rule);
    add_production("MatchStar", {"CompleteGroup", "Star"}, regex_match_zero_or_more_rule);
    add_production("MatchPlus", {"CompleteGroup", "Plus"}, regex_match_one_or_more_rule);
    add_production(
            "MatchExact",
            {"CompleteGroup", "Lbrace", "Integer", "Rbrace"},
            regex_match_exactly_rule
    );
    add_production(
            "MatchRange",
            {"CompleteGroup", "Lbrace", "Integer", "Comma", "Integer", "Rbrace"},
            regex_match_range_rule
    );
    add_production("CompleteGroup", {"IncompleteGroup", "Rbracket"}, regex_identity_rule);
    add_production("CompleteGroup", {"Literal"}, regex_identity_rule);
    add_production("CompleteGroup", {"Digit"}, regex_identity_rule);
    add_production("CompleteGroup", {"Wildcard"}, regex_identity_rule);
    add_production("CompleteGroup", {"WhiteSpace"}, regex_identity_rule);
    add_production(
            "IncompleteGroup",
            {"IncompleteGroup", "LiteralRange"},
            regex_add_range_existing_group_rule
    );
    add_production(
            "IncompleteGroup",
            {"IncompleteGroup", "Digit"},
            regex_add_range_existing_group_rule
    );
    add_production(
            "IncompleteGroup",
            {"IncompleteGroup", "Literal"},
            regex_add_literal_existing_group_rule
    );
    add_production(
            "IncompleteGroup",
            {"IncompleteGroup", "WhiteSpace"},
            regex_add_literal_existing_group_rule
    );
    add_production("IncompleteGroup", {"Lbracket", "LiteralRange"}, regex_add_range_new_group_rule);
    add_production("IncompleteGroup", {"Lbracket", "Digit"}, regex_add_range_new_group_rule);
    add_production("IncompleteGroup", {"Lbracket", "Literal"}, regex_add_literal_new_group_rule);
    add_production("IncompleteGroup", {"Lbracket", "WhiteSpace"}, regex_add_literal_new_group_rule);
    add_production("IncompleteGroup", {"Lbracket", "Hat"}, regex_complement_incomplete_group_rule);
    add_production("LiteralRange", {"Literal", "Dash", "Literal"}, regex_range_rule);
    add_production("Literal", {"Backslash", "t"}, regex_tab_rule);
    add_production("Literal", {"Backslash", "n"}, regex_newline_rule);
    add_production("Literal", {"Backslash", "v"}, regex_vertical_tab_rule);
    add_production("Literal", {"Backslash", "f"}, regex_form_feed_rule);
    add_production("Literal", {"Backslash", "r"}, regex_char_return_rule);
    add_production("Literal", {"Space"}, regex_literal_rule);
    add_production("Literal", {"Bang"}, regex_literal_rule);
    add_production("Literal", {"Quotation"}, regex_literal_rule);
    add_production("Literal", {"Hash"}, regex_literal_rule);
    add_production("Literal", {"DollarSign"}, regex_literal_rule);
    add_production("Literal", {"Percent"}, regex_literal_rule);
    add_production("Literal", {"Ampersand"}, regex_literal_rule);
    add_production("Literal", {"Apostrophe"}, regex_literal_rule);
    add_production("Literal", {"Backslash", "Lparen"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Rparen"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Star"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Plus"}, regex_cancel_literal_rule);
    add_production("Literal", {"Comma"}, regex_literal_rule);
    add_production("Literal", {"Backslash", "Dash"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Dot"}, regex_cancel_literal_rule);
    add_production("Literal", {"ForwardSlash"}, regex_literal_rule);
    add_production("Literal", {"AlphaNumeric"}, regex_literal_rule);
    add_production("Literal", {"Colon"}, regex_literal_rule);
    add_production("Literal", {"SemiColon"}, regex_literal_rule);
    add_production("Literal", {"Equal"}, regex_literal_rule);
    add_production("Literal", {"At"}, regex_literal_rule);
    add_production("Literal", {"Backslash", "Lbracket"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Backslash"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Rbracket"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Hat"}, regex_cancel_literal_rule);
    add_production("Literal", {"Underscore"}, regex_literal_rule);
    add_production("Literal", {"Backtick"}, regex_literal_rule);
    add_production("Literal", {"Backslash", "Lbrace"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Vbar"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Rbrace"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Langle"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "Rangle"}, regex_cancel_literal_rule);
    add_production("Literal", {"Backslash", "QuestionMark"}, regex_cancel_literal_rule);
    add_production("Literal", {"Tilde"}, regex_literal_rule);
    add_production(
            "Literal",
            {"Lparen", "QuestionMark", "Langle", "Identifier", "Rangle", "Regex", "Rparen"},
            regex_capture_rule
    );
    add_production("Literal", {"Lparen", "Regex", "Rparen"}, regex_middle_identity_rule);
    for (auto const& [special_regex_char, special_regex_name] : m_special_regex_characters) {
        std::ignore = special_regex_char;
        add_production("Literal", {"Backslash", special_regex_name}, regex_cancel_literal_rule);
    }
    add_production("Integer", {"Integer", "Numeric"}, regex_existing_integer_rule);
    add_production("Integer", {"Numeric"}, regex_new_integer_rule);
    add_production("Digit", {"Backslash", "d"}, regex_digit_rule);
    add_production("Wildcard", {"Dot"}, regex_wildcard_rule);
    add_production("WhiteSpace", {"Backslash", "s"}, regex_white_space_rule);
}
}  // namespace log_surgeon

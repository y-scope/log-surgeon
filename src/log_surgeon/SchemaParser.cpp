#include "SchemaParser.hpp"

#include <cmath>
#include <memory>
#include <span>
#include <stdexcept>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/FileReader.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/LALR1Parser.hpp>
#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/utils.hpp>

using RegexASTByte
        = log_surgeon::finite_automata::RegexAST<log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTGroupByte = log_surgeon::finite_automata::RegexASTGroup<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTIntegerByte = log_surgeon::finite_automata::RegexASTInteger<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTLiteralByte = log_surgeon::finite_automata::RegexASTLiteral<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTMultiplicationByte = log_surgeon::finite_automata::RegexASTMultiplication<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTOrByte
        = log_surgeon::finite_automata::RegexASTOr<log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTCatByte = log_surgeon::finite_automata::RegexASTCat<
        log_surgeon::finite_automata::RegexNFAByteState>;

using std::make_unique;
using std::string;
using std::unique_ptr;

namespace log_surgeon {
SchemaParser::SchemaParser() {
    add_lexical_rules();
    add_productions();
    generate();
}

auto SchemaParser::generate_schema_ast(Reader& reader) -> unique_ptr<SchemaAST> {
    NonTerminal nonterminal = parse(reader);
    std::unique_ptr<SchemaAST> schema_ast(
            dynamic_cast<SchemaAST*>(nonterminal.get_parser_ast().release())
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
        int code{static_cast<std::underlying_type_t<ErrorCode>>(error_code)};
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

auto SchemaParser::try_schema_string(string const& schema_string) -> unique_ptr<SchemaAST> {
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
    string r1 = m->token_cast(0)->to_string();
    return make_unique<IdentifierAST>(IdentifierAST(r1[0]));
}

static auto existing_identifier_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(0)->get_parser_ast();
    auto* r1_ptr = dynamic_cast<IdentifierAST*>(r1.get());
    string r2 = m->token_cast(1)->to_string();
    r1_ptr->add_character(r2[0]);
    return std::move(r1);
}

static auto schema_var_rule(NonTerminal* m) -> unique_ptr<SchemaVarAST> {
    auto* r2 = dynamic_cast<IdentifierAST*>(m->non_terminal_cast(1)->get_parser_ast().get());
    Token* colon_token = m->token_cast(2);
    auto& r4 = m->non_terminal_cast(3)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return make_unique<SchemaVarAST>(r2->m_name, std::move(r4), colon_token->m_line);
}

static auto new_schema_rule(NonTerminal* /* m */) -> unique_ptr<SchemaAST> {
    return make_unique<SchemaAST>();
}

static auto new_schema_rule_with_var(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(0)->get_parser_ast();
    unique_ptr<SchemaAST> schema_ast = make_unique<SchemaAST>();
    schema_ast->add_schema_var(std::move(r1));
    return schema_ast;
}

static auto new_schema_rule_with_delimiters(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(2)->get_parser_ast();
    unique_ptr<SchemaAST> schema_ast = make_unique<SchemaAST>();
    schema_ast->add_delimiters(std::move(r1));
    return schema_ast;
}

static auto existing_schema_rule_with_delimiter(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(0)->get_parser_ast();
    std::unique_ptr<SchemaAST> schema_ast(dynamic_cast<SchemaAST*>(r1.release()));
    unique_ptr<ParserAST>& r5 = m->non_terminal_cast(4)->get_parser_ast();
    schema_ast->add_delimiters(std::move(r5));
    return schema_ast;
}

auto SchemaParser::existing_schema_rule(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(0)->get_parser_ast();
    std::unique_ptr<SchemaAST> schema_ast(dynamic_cast<SchemaAST*>(r1.release()));
    unique_ptr<ParserAST>& r2 = m->non_terminal_cast(2)->get_parser_ast();
    schema_ast->add_schema_var(std::move(r2));
    // Can reset the buffers at this point and allow reading
    if (NonTerminal::m_next_children_start > cSizeOfAllChildren / 2) {
        NonTerminal::m_next_children_start = 0;
    }
    return schema_ast;
}

static auto identity_rule_ParserASTSchema(NonTerminal* m) -> unique_ptr<SchemaAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(0)->get_parser_ast();
    std::unique_ptr<SchemaAST> schema_ast(dynamic_cast<SchemaAST*>(r1.release()));
    return schema_ast;
}

using ParserValueRegex = ParserValue<unique_ptr<RegexASTByte>>;

static auto regex_identity_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(new ParserValueRegex(
            std::move(m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>())
    ));
}

static auto regex_cat_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto& r2 = m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTCatByte(std::move(r1), std::move(r2)))
    ));
}

static auto regex_or_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto& r2 = m->non_terminal_cast(2)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTOrByte(std::move(r1), std::move(r2)))
    ));
}

static auto regex_match_zero_or_more_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTMultiplicationByte(std::move(r1), 0, 0))
    ));
}

static auto regex_match_one_or_more_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTMultiplicationByte(std::move(r1), 1, 0))
    ));
}

static auto regex_match_exactly_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r3 = m->non_terminal_cast(2)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r3_ptr = dynamic_cast<RegexASTIntegerByte*>(r3.get());
    uint32_t reps = 0;
    uint32_t r3_size = r3_ptr->get_digits().size();
    for (uint32_t i = 0; i < r3_size; i++) {
        reps += r3_ptr->get_digit(i) * (uint32_t)pow(10, r3_size - i - 1);
    }
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTMultiplicationByte(std::move(r1), reps, reps))
    ));
}

static auto regex_match_range_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r3 = m->non_terminal_cast(2)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r3_ptr = dynamic_cast<RegexASTIntegerByte*>(r3.get());
    uint32_t min = 0;
    uint32_t r3_size = r3_ptr->get_digits().size();
    for (uint32_t i = 0; i < r3_size; i++) {
        min += r3_ptr->get_digit(i) * (uint32_t)pow(10, r3_size - i - 1);
    }
    auto& r5 = m->non_terminal_cast(4)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r5_ptr = dynamic_cast<RegexASTIntegerByte*>(r5.get());
    uint32_t max = 0;
    uint32_t r5_size = r5_ptr->get_digits().size();
    for (uint32_t i = 0; i < r5_size; i++) {
        max += r5_ptr->get_digit(i) * (uint32_t)pow(10, r5_size - i - 1);
    }
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTMultiplicationByte(std::move(r1), min, max))
    ));
}

static auto regex_add_literal_existing_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto& r2 = m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r1_ptr = dynamic_cast<RegexASTGroupByte*>(r1.get());
    auto* r2_ptr = dynamic_cast<RegexASTLiteralByte*>(r2.get());
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTGroupByte(r1_ptr, r2_ptr)))
    );
}

static auto regex_add_range_existing_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto& r2 = m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r1_ptr = dynamic_cast<RegexASTGroupByte*>(r1.get());
    auto* r2_ptr = dynamic_cast<RegexASTGroupByte*>(r2.get());
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTGroupByte(r1_ptr, r2_ptr)))
    );
}

static auto regex_add_literal_new_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r2 = m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r2_ptr = dynamic_cast<RegexASTLiteralByte*>(r2.get());
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTGroupByte(r2_ptr)))
    );
}

static auto regex_add_range_new_group_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r2 = m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r2_ptr = dynamic_cast<RegexASTGroupByte*>(r2.get());
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTGroupByte(r2_ptr)))
    );
}

static auto regex_complement_incomplete_group_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(new ParserValueRegex(make_unique<RegexASTGroupByte>()));
}

static auto regex_range_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto& r2 = m->non_terminal_cast(2)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r1_ptr = dynamic_cast<RegexASTLiteralByte*>(r1.get());
    auto* r2_ptr = dynamic_cast<RegexASTLiteralByte*>(r2.get());
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTGroupByte(r1_ptr, r2_ptr)))
    );
}

static auto regex_middle_identity_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(new ParserValueRegex(
            std::move(m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>())
    ));
}

static auto regex_literal_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    Token* token = m->token_cast(0);
    assert(token->to_string().size() == 1);
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTLiteralByte(token->to_string()[0]))
    ));
}

static auto regex_cancel_literal_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    Token* token = m->token_cast(1);
    assert(token->to_string().size() == 1);
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTLiteralByte(token->to_string()[0]))
    ));
}

static auto regex_existing_integer_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r2 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r2_ptr = dynamic_cast<RegexASTIntegerByte*>(r2.get());
    Token* token = m->token_cast(1);
    assert(token->to_string().size() == 1);
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTIntegerByte(r2_ptr, token->to_string()[0]))
    ));
}

static auto regex_new_integer_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    Token* token = m->token_cast(0);
    assert(token->to_string().size() == 1);
    return unique_ptr<ParserAST>(new ParserValueRegex(
            unique_ptr<RegexASTByte>(new RegexASTIntegerByte(token->to_string()[0]))
    ));
}

static auto regex_digit_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTGroupByte('0', '9')))
    );
}

static auto regex_wildcard_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    unique_ptr<RegexASTGroupByte> regex_wildcard = make_unique<RegexASTGroupByte>(0, cUnicodeMax);
    regex_wildcard->set_is_wildcard_true();
    return unique_ptr<ParserAST>(new ParserValueRegex(std::move(regex_wildcard)));
}

static auto regex_vertical_tab_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTLiteralByte('\v')))
    );
}

static auto regex_form_feed_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTLiteralByte('\f')))
    );
}

static auto regex_tab_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTLiteralByte('\t')))
    );
}

static auto regex_char_return_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTLiteralByte('\r')))
    );
}

static auto regex_newline_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(new RegexASTLiteralByte('\n')))
    );
}

static auto regex_white_space_rule(NonTerminal* /* m */) -> unique_ptr<ParserAST> {
    unique_ptr<RegexASTGroupByte> regex_ast_group = make_unique<RegexASTGroupByte>(
            RegexASTGroupByte({' ', '\t', '\r', '\n', '\v', '\f'})
    );
    return unique_ptr<ParserAST>(
            new ParserValueRegex(unique_ptr<RegexASTByte>(std::move(regex_ast_group)))
    );
}

static auto existing_delimiter_string_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    unique_ptr<ParserAST>& r1 = m->non_terminal_cast(0)->get_parser_ast();
    auto& r2 = m->non_terminal_cast(1)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    auto* r1_ptr = dynamic_cast<DelimiterStringAST*>(r1.get());
    uint32_t character = dynamic_cast<RegexASTLiteralByte*>(r2.get())->get_character();
    r1_ptr->add_delimiter(character);
    return std::move(r1);
}

static auto new_delimiter_string_rule(NonTerminal* m) -> unique_ptr<ParserAST> {
    auto& r1 = m->non_terminal_cast(0)->get_parser_ast()->get<unique_ptr<RegexASTByte>>();
    uint32_t character = dynamic_cast<RegexASTLiteralByte*>(r1.get())->get_character();
    return make_unique<DelimiterStringAST>(character);
}

void SchemaParser::add_lexical_rules() {
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
    add_token("Lparen", '(');
    add_token("Rparen", ')');
    add_token("Star", '*');
    add_token("Plus", '+');
    add_token("Comma", ',');
    add_token("Dash", '-');
    add_token("Dot", '.');
    add_token("ForwardSlash", '/');
    add_token_group("Numeric", make_unique<RegexASTGroupByte>('0', '9'));
    add_token("Colon", ':');
    add_token("SemiColon", ';');
    add_token("LAngle", '<');
    add_token("Equal", '=');
    add_token("RAngle", '>');
    add_token("QuestionMark", '?');
    add_token("At", '@');
    add_token_group("AlphaNumeric", make_unique<RegexASTGroupByte>('a', 'z'));
    add_token_group("AlphaNumeric", make_unique<RegexASTGroupByte>('A', 'Z'));
    add_token_group("AlphaNumeric", make_unique<RegexASTGroupByte>('0', '9'));
    add_token("Lbracket", '[');
    add_token("Backslash", '\\');
    add_token("Rbracket", ']');
    add_token("Hat", '^');
    add_token("Underscore", '_');
    add_token("Backtick", '`');
    add_token("Lbrace", '{');
    add_token("Vbar", '|');
    add_token("Rbrace", '}');
    add_token("Tilde", '~');
    add_token("d", 'd');
    add_token("s", 's');
    add_token("n", 'n');
    add_token("r", 'r');
    add_token("t", 't');
    add_token("f", 'f');
    add_token("v", 'v');
    add_token_chain("Delimiters", "delimiters");
    // default constructs to a m_negate group
    unique_ptr<RegexASTGroupByte> comment_characters = make_unique<RegexASTGroupByte>();
    comment_characters->add_literal('\r');
    comment_characters->add_literal('\n');
    add_token_group("CommentCharacters", std::move(comment_characters));
}

void SchemaParser::add_productions() {
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
    add_production("Identifier", {"Identifier", "AlphaNumeric"}, existing_identifier_rule);
    add_production("Identifier", {"AlphaNumeric"}, new_identifier_rule);
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
    add_production("Literal", {"LAngle"}, regex_literal_rule);
    add_production("Literal", {"Equal"}, regex_literal_rule);
    add_production("Literal", {"RAngle"}, regex_literal_rule);
    add_production("Literal", {"QuestionMark"}, regex_literal_rule);
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
    add_production("Literal", {"Tilde"}, regex_literal_rule);
    add_production("Literal", {"Lparen", "Regex", "Rparen"}, regex_middle_identity_rule);
    add_production("Integer", {"Integer", "Numeric"}, regex_existing_integer_rule);
    add_production("Integer", {"Numeric"}, regex_new_integer_rule);
    add_production("Digit", {"Backslash", "d"}, regex_digit_rule);
    add_production("Wildcard", {"Dot"}, regex_wildcard_rule);
    add_production("WhiteSpace", {"Backslash", "s"}, regex_white_space_rule);
}
}  // namespace log_surgeon

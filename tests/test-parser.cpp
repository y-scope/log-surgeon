#include <codecvt>
#include <cstdint>
#include <locale>
#include <memory>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>
#include <fmt/core.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/LogParser.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/types.hpp>

using log_surgeon::capture_id_t;
using log_surgeon::finite_automata::PrefixTree;
using log_surgeon::lexers::ByteLexer;
using log_surgeon::LogParser;
using log_surgeon::Schema;
using log_surgeon::SchemaAST;
using log_surgeon::SymbolId;
using std::codecvt_utf8;
using std::make_unique;
using std::string;
using std::string_view;
using std::u32string;
using std::unordered_map;
using std::vector;
using std::wstring_convert;

using RegexASTCatByte
        = log_surgeon::finite_automata::RegexASTCat<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTCaptureByte
        = log_surgeon::finite_automata::RegexASTCapture<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTGroupByte
        = log_surgeon::finite_automata::RegexASTGroup<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTLiteralByte
        = log_surgeon::finite_automata::RegexASTLiteral<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTMultiplicationByte = log_surgeon::finite_automata::RegexASTMultiplication<
        log_surgeon::finite_automata::ByteNfaState>;
using RegexASTOrByte
        = log_surgeon::finite_automata::RegexASTOr<log_surgeon::finite_automata::ByteNfaState>;
using log_surgeon::rule_id_t;
using log_surgeon::SchemaVarAST;

namespace {
/**
 * Parses the given input and verifies the next token is for the given rule name.
 *
 * If the rule has captures, verifies the captures are in the right place.
 *
 * @param log_parser The log parser to scan the input with.
 * @param input_buffer The input buffer to lex.
 * @param expected_input_match The part of the input that matches this token.
 * @param rule_name The expected symbol to match (empty for uncaught string).
 * @param expected_capture_map The expected start and end position of each capture.
 */
auto parse_and_validate_next_token(
        LogParser& log_parser,
        log_surgeon::ParserInputBuffer& input_buffer,
        std::string_view expected_input_match,
        std::string_view rule_name,
        std::map<
                capture_id_t,
                std::pair<
                        std::vector<PrefixTree::position_t>,
                        std::vector<PrefixTree::position_t>>> const& expected_capture_map
) -> void;

/**
 * Parses the given input and verifies the output is a sequence of tokens matching the expected
 * tokens.
 *
 * If any rule has captures, verifies the captures are in the right place.
 *
 * @param log_parser The log parser to scan the input with.
 * @param input The input to lex.
 * @param expected_test_sequence A list of expected matches, where each match is a tuple containing:
 * - The substring of the input that matches the current token,
 * - The rule name of the current token (empty for uncaught string),
 * - The start and end position of each capture.
 */
auto parse_and_validate_sequence(
        LogParser& log_parser,
        std::string_view input,
        std::vector<std::tuple<
                std::string_view,
                std::string_view,
                std::map<
                        capture_id_t,
                        std::pair<
                                std::vector<PrefixTree::position_t>,
                                std::vector<PrefixTree::position_t>>>>> const&
                expected_test_sequence
) -> void;

/**
 * @param map The map to serialize.
 * @return The serialized map.
 */
[[nodiscard]] auto serialize_id_symbol_map(unordered_map<rule_id_t, string> const& map) -> string;

auto parse_and_validate_next_token(
        LogParser& log_parser,
        log_surgeon::ParserInputBuffer& input_buffer,
        std::string_view expected_input_match,
        std::string_view rule_name,
        std::map<
                capture_id_t,
                std::pair<
                        std::vector<PrefixTree::position_t>,
                        std::vector<PrefixTree::position_t>>> const& expected_capture_map
) -> void {
    ByteLexer& lexer{log_parser.m_lexer};
    CAPTURE(rule_name);
    CAPTURE(expected_input_match);

    auto [err, optional_token1]{lexer.scan(input_buffer)};
    REQUIRE(log_surgeon::ErrorCode::Success == err);
    REQUIRE(optional_token1.has_value());
    if (false == optional_token1.has_value()) {
        return;
    }
    auto token{optional_token1.value()};
    auto token_type{token.m_type_ids_ptr->at(0)};
    CAPTURE(token.to_string_view());
    CAPTURE(*token.m_type_ids_ptr);
    REQUIRE(nullptr != token.m_type_ids_ptr);
    if (rule_name.empty()) {
        REQUIRE(log_surgeon::cTokenUncaughtString == lexer.m_id_symbol.at(token_type));
    } else {
        REQUIRE(rule_name == lexer.m_id_symbol.at(token_type));
    }
    REQUIRE(expected_input_match == token.to_string_view());

    if (false == expected_capture_map.empty()) {
        auto optional_capture_ids{lexer.get_capture_ids_from_rule_id(token_type)};
        REQUIRE(optional_capture_ids.has_value());
        if (false == optional_capture_ids.has_value()) {
            return;
        }
        for (auto const capture_id : optional_capture_ids.value()) {
            REQUIRE(expected_capture_map.contains(capture_id));
            auto optional_reg_ids{lexer.get_reg_ids_from_capture_id(capture_id)};
            REQUIRE(optional_reg_ids.has_value());
            if (false == optional_reg_ids.has_value()) {
                return;
            }
            auto const [start_reg_id, end_reg_id]{optional_reg_ids.value()};
            auto const actual_start_positions{token.get_reg_positions(start_reg_id)};
            auto const actual_end_positions{token.get_reg_positions(end_reg_id)};
            auto const [expected_start_positions, expected_end_positions]{
                    expected_capture_map.at(capture_id)
            };
            REQUIRE(expected_start_positions == actual_start_positions);
            REQUIRE(expected_end_positions == actual_end_positions);
        }
    }
}

auto parse_and_validate_sequence(
        LogParser& log_parser,
        std::string_view input,
        std::vector<std::tuple<
                std::string_view,
                std::string_view,
                std::map<
                        capture_id_t,
                        std::pair<
                                std::vector<PrefixTree::position_t>,
                                std::vector<PrefixTree::position_t>>>>> const&
                expected_test_sequence
) -> void {
    auto& lexer{log_parser.m_lexer};
    lexer.reset();
    CAPTURE(serialize_id_symbol_map(lexer.m_id_symbol));

    log_surgeon::ParserInputBuffer input_buffer;
    string token_string{input};
    input_buffer.set_storage(token_string.data(), token_string.size(), 0, true);
    lexer.prepend_start_of_file_char(input_buffer);

    CAPTURE(input);
    for (auto const& [expected_input_match, rule_name, captures] : expected_test_sequence) {
        parse_and_validate_next_token(
                log_parser,
                input_buffer,
                expected_input_match,
                rule_name,
                captures
        );
    }

    // Make sure it finishes on cTokenEnd
    auto [err, optional_token2]{lexer.scan(input_buffer)};
    REQUIRE(log_surgeon::ErrorCode::Success == err);
    REQUIRE(optional_token2.has_value());
    if (false == optional_token2.has_value()) {
        return;
    }
    auto token{optional_token2.value()};
    REQUIRE(nullptr != token.m_type_ids_ptr);
    CAPTURE(token.to_string_view());
    CAPTURE(*token.m_type_ids_ptr);
    REQUIRE(1 == token.m_type_ids_ptr->size());
    REQUIRE(log_surgeon::cTokenEnd == lexer.m_id_symbol.at(token.m_type_ids_ptr->at(0)));
    REQUIRE(token.to_string_view().empty());
}

auto serialize_id_symbol_map(unordered_map<rule_id_t, string> const& map) -> string {
    string serialized_map;
    for (auto const& [id, symbol] : map) {
        serialized_map += fmt::format("{}->{},", id, symbol);
    }
    return serialized_map;
}
}  // namespace

TEST_CASE("Test the Schema class", "[Schema]") {
    SECTION("Add a number variable to schema") {
        Schema schema;
        string const var_name = "myNumber";
        string const var_schema = var_name + string(":") + string("123");
        schema.add_variable(string_view(var_schema), -1);

        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars.at(0);
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        REQUIRE_NOTHROW([&]() {
            (void)dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr);
        }());
    }

    SECTION("Add a capture variable to schema") {
        Schema schema;
        std::string const var_name = "capture";
        string const var_schema = var_name + string(":") + string("u(?<uID>[0-9]+)");
        schema.add_variable(var_schema, -1);

        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars.at(0);
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        auto const* regex_ast_cat_ptr
                = dynamic_cast<RegexASTCatByte*>(schema_var_ast.m_regex_ptr.get());
        REQUIRE(nullptr != regex_ast_cat_ptr);
        REQUIRE(nullptr != regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_cat_ptr->get_right());

        auto const* regex_ast_literal
                = dynamic_cast<RegexASTLiteralByte const*>(regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_literal);
        REQUIRE('u' == regex_ast_literal->get_character());

        auto const* regex_ast_capture
                = dynamic_cast<RegexASTCaptureByte const*>(regex_ast_cat_ptr->get_right());
        REQUIRE(nullptr != regex_ast_capture);
        REQUIRE("uID" == regex_ast_capture->get_capture_name());

        auto const* regex_ast_multiplication_ast = dynamic_cast<RegexASTMultiplicationByte*>(
                regex_ast_capture->get_capture_regex_ast().get()
        );
        REQUIRE(nullptr != regex_ast_multiplication_ast);
        REQUIRE(1 == regex_ast_multiplication_ast->get_min());
        REQUIRE(0 == regex_ast_multiplication_ast->get_max());
        REQUIRE(regex_ast_multiplication_ast->is_infinite());

        auto const* regex_ast_group_ast
                = dynamic_cast<RegexASTGroupByte*>(regex_ast_multiplication_ast->get_operand().get()
                );
        REQUIRE(false == regex_ast_group_ast->is_wildcard());
        REQUIRE(1 == regex_ast_group_ast->get_ranges().size());
        REQUIRE('0' == regex_ast_group_ast->get_ranges().at(0).first);
        REQUIRE('9' == regex_ast_group_ast->get_ranges().at(0).second);
    }
}

TEST_CASE("Test Lexer without capture groups", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarName{"myVar"};
    constexpr string_view cVarSchema{"myVar:userID=123"};
    constexpr string_view cTokenString1{"userID=123"};
    constexpr string_view cTokenString2{"userID=234"};
    constexpr string_view cTokenString3{"123"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);

    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    CAPTURE(cVarSchema);
    parse_and_validate_sequence(log_parser, cTokenString1, {{cTokenString1, cVarName, {}}});
    parse_and_validate_sequence(
            log_parser,
            cTokenString2,
            {{cTokenString2, log_surgeon::cTokenUncaughtString, {}}}
    );
    parse_and_validate_sequence(
            log_parser,
            cTokenString3,
            {{cTokenString3, log_surgeon::cTokenUncaughtString, {}}}
    );
}

TEST_CASE("Test Lexer with capture groups", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarName{"myVar"};
    constexpr string_view cCaptureName{"uid"};
    constexpr string_view cVarSchema{"myVar:userID=(?<uid>123)"};
    constexpr string_view cTokenString1{"userID=123"};
    constexpr string_view cTokenString2{"userID=234"};
    constexpr string_view cTokenString3{"123"};
    std::pair<std::vector<PrefixTree::position_t>, std::vector<PrefixTree::position_t>> const
            capture_positions{{7}, {10}};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    ByteLexer& lexer{log_parser.m_lexer};

    string const var_name{cVarName};
    REQUIRE(lexer.m_symbol_id.contains(var_name));

    string const capture_name{cCaptureName};
    REQUIRE(lexer.m_symbol_id.contains(capture_name));

    auto const optional_capture_ids{lexer.get_capture_ids_from_rule_id(lexer.m_symbol_id.at(var_name
    ))};
    REQUIRE(optional_capture_ids.has_value());
    REQUIRE(1 == optional_capture_ids.value().size());
    REQUIRE(lexer.m_symbol_id.at(capture_name) == optional_capture_ids.value().at(0));

    auto const optional_tag_id_pair{
            lexer.get_tag_id_pair_from_capture_id(optional_capture_ids.value().at(0))
    };
    REQUIRE(optional_tag_id_pair.has_value());
    REQUIRE(std::make_pair(0U, 1U) == optional_tag_id_pair.value());

    auto reg_id0{lexer.get_reg_id_from_tag_id(optional_tag_id_pair.value().first)};
    auto reg_id1{lexer.get_reg_id_from_tag_id(optional_tag_id_pair.value().second)};
    REQUIRE(2u == reg_id0.value());
    REQUIRE(3u == reg_id1.value());

    CAPTURE(cVarSchema);
    parse_and_validate_sequence(
            log_parser,
            cTokenString1,
            {{cTokenString1, cVarName, {{lexer.m_symbol_id.at(capture_name), capture_positions}}}}
    );
    parse_and_validate_sequence(
            log_parser,
            cTokenString2,
            {{cTokenString2, log_surgeon::cTokenUncaughtString, {}}}
    );
    parse_and_validate_sequence(
            log_parser,
            cTokenString3,
            {{cTokenString3, log_surgeon::cTokenUncaughtString, {}}}
    );
}

TEST_CASE("Test CLP default schema", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    string const capture_name{"val"};
    constexpr string_view cVarName1{"firstTimestamp"};
    constexpr string_view cVarSchema1{
            R"(timestamp:[0-9]{4}\-[0-9]{2}\-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}[,\.][0-9]{0,3})"
    };
    constexpr string_view cVarName2{"int"};
    constexpr string_view cVarSchema2{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cVarName3{"float"};
    constexpr string_view cVarSchema3{R"(float:\-{0,1}[0-9]+\.[0-9]+)"};
    constexpr string_view cVarName4{"hex"};
    constexpr string_view cVarSchema4{R"(hex:[a-fA-F]+)"};
    constexpr string_view cVarName5{"equals"};
    constexpr string_view cVarSchema5{R"(equals:[^ \r\n=]+=(?<val>[^ \r\n]*[A-Za-z0-9][^ \r\n]*))"};
    constexpr string_view cVarName6{"hasNumber"};
    constexpr string_view cVarSchema6{R"(hasNumber:={0,1}[^ \r\n=]*\d[^ \r\n=]*={0,1})"};

    constexpr string_view cTokenString1{"2012-12-12 12:12:12.123"};
    constexpr string_view cTokenString2{"123"};
    constexpr string_view cTokenString3{"123.123"};
    constexpr string_view cTokenString4{"abc"};
    constexpr string_view cTokenString5{"userID=123"};
    constexpr string_view cTokenString6{"user123"};
    std::pair<std::vector<PrefixTree::position_t>, std::vector<PrefixTree::position_t>> const
            capture_positions{{7}, {10}};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema1, -1);
    schema.add_variable(cVarSchema2, -1);
    schema.add_variable(cVarSchema3, -1);
    schema.add_variable(cVarSchema4, -1);
    schema.add_variable(cVarSchema5, -1);
    schema.add_variable(cVarSchema6, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate_sequence(log_parser, cTokenString1, {{cTokenString1, cVarName1, {}}});
    parse_and_validate_sequence(log_parser, cTokenString2, {{cTokenString2, cVarName2, {}}});
    parse_and_validate_sequence(log_parser, cTokenString3, {{cTokenString3, cVarName3, {}}});
    parse_and_validate_sequence(log_parser, cTokenString4, {{cTokenString4, cVarName4, {}}});
    parse_and_validate_sequence(
            log_parser,
            cTokenString5,
            {{cTokenString5, cVarName5, {{log_parser.m_lexer.m_symbol_id.at(capture_name), capture_positions}}}}
    );
    parse_and_validate_sequence(log_parser, cTokenString6, {{cTokenString6, cVarName6, {}}});
}

TEST_CASE("Test delimited variables", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    string const capture_name{"val"};
    constexpr string_view cVarName1{"function"};
    constexpr string_view cVarSchema1{"function:[A-Za-z]+::[A-Za-z]+1"};
    constexpr string_view cVarName2{"path"};
    constexpr string_view cVarSchema2{R"(path:[a-zA-Z0-9_/\.\-]+/[a-zA-Z0-9_/\.\-]+)"};
    constexpr string_view cTokenString1{"Word GeExecutor::Initialize1"};
    constexpr string_view cTokenString2{"word::my/path/to/file.txt"};
    constexpr string_view cTokenString3{"GeExecutor::Initialize"};
    constexpr string_view cTokenString4{"::GeExecutor::Initialize1"};
    constexpr string_view cTokenString5{"folder/file-op71"};
    constexpr string_view cTokenString6{"[WARNING] PARALLEL:2024 [folder/file.cc:150] insert "
                                        "node:folder/file-op7, id:7 and folder/file-op8, id:8"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema1, -1);
    schema.add_variable(cVarSchema2, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    CAPTURE(cVarSchema1);
    parse_and_validate_sequence(
            log_parser,
            cTokenString1,
            {{"Word", "", {}}, {" GeExecutor::Initialize1", cVarName1, {}}}
    );

    CAPTURE(cVarSchema2);
    parse_and_validate_sequence(
            log_parser,
            cTokenString2,
            {{"word", "", {}}, {":", "", {}}, {":my/path/to/file.txt", cVarName2, {}}}
    );

    parse_and_validate_sequence(
            log_parser,
            cTokenString3,
            {{"GeExecutor", "", {}}, {":", "", {}}, {":Initialize", "", {}}}
    );

    parse_and_validate_sequence(
            log_parser,
            cTokenString4,
            {{":", "", {}}, {":GeExecutor::Initialize1", cVarName1, {}}}
    );

    parse_and_validate_sequence(log_parser, cTokenString5, {{cTokenString5, cVarName2, {}}});

    parse_and_validate_sequence(
            log_parser,
            cTokenString6,
            {{"[WARNING]", "", {}},
             {" PARALLEL", "", {}},
             {":2024", "", {}},
             {" ", "", {}},
             {"[folder/file.cc", "path", {}},
             {":150]", "", {}},
             {" insert", "", {}},
             {" node", "", {}},
             {":folder/file-op7", "path", {}},
             {",", "", {}},
             {" id", "", {}},
             {":7", "", {}},
             {" and", "", {}},
             {" folder/file-op8", "path", {}},
             {",", "", {}},
             {" id", "", {}},
             {":8", "", {}}}
    );
}

TEST_CASE("Test delimited variables", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    string const capture_name{"val"};
    constexpr string_view cVarName1{"function"};
    constexpr string_view cVarSchema1{"function:[A-Za-z]+::[A-Za-z]+1"};
    constexpr string_view cVarName2{"path"};
    constexpr string_view cVarSchema2{R"(path:[a-zA-Z0-9_/\.\-]+/[a-zA-Z0-9_/\.\-]+)"};
    constexpr string_view cTokenString1{"Word App::Action1"};
    constexpr string_view cTokenString2{"word::my/path/to/file.txt"};
    constexpr string_view cTokenString3{"App::Action"};
    constexpr string_view cTokenString4{"::App::Action1"};
    constexpr string_view cTokenString5{"folder/file-op71"};
    constexpr string_view cTokenString6{"[WARNING] PARALLEL:2024 [folder/file.cc:150] insert "
                                        "node:folder/file-op7, id:7 and folder/file-op8, id:8"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema1, -1);
    schema.add_variable(cVarSchema2, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    CAPTURE(cVarSchema1);
    parse_and_validate_sequence(
            log_parser,
            cTokenString1,
            {{"Word", "", {}}, {" App::Action1", cVarName1, {}}}
    );

    CAPTURE(cVarSchema2);
    parse_and_validate_sequence(
            log_parser,
            cTokenString2,
            {{"word", "", {}}, {":", "", {}}, {":my/path/to/file.txt", cVarName2, {}}}
    );

    parse_and_validate_sequence(
            log_parser,
            cTokenString3,
            {{"App", "", {}}, {":", "", {}}, {":Action", "", {}}}
    );

    parse_and_validate_sequence(
            log_parser,
            cTokenString4,
            {{":", "", {}}, {":App::Action1", cVarName1, {}}}
    );

    parse_and_validate_sequence(log_parser, cTokenString5, {{cTokenString5, cVarName2, {}}});

    parse_and_validate_sequence(
            log_parser,
            cTokenString6,
            {{"[WARNING]", "", {}},
             {" PARALLEL", "", {}},
             {":2024", "", {}},
             {" ", "", {}},
             {"[folder/file.cc", "path", {}},
             {":150]", "", {}},
             {" insert", "", {}},
             {" node", "", {}},
             {":folder/file-op7", "path", {}},
             {",", "", {}},
             {" id", "", {}},
             {":7", "", {}},
             {" and", "", {}},
             {" folder/file-op8", "path", {}},
             {",", "", {}},
             {" id", "", {}},
             {":8", "", {}}}
    );
}

TEST_CASE(
        "Test integer after static-text at start of newline when previous line ends in a variable",
        "[Lexer]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567\nWord 1234567"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cRule, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate_sequence(
            log_parser,
            cInput,
            // NOTE: LogParser will realize "\nWord" is the start of a new log message
            {{"1234567", "int", {}},
             {"\n", "newLine", {}},
             {"Word", "", {}},
             {" 1234567", "int", {}}}
    );
}

TEST_CASE(
        "Test integer after static-text at start of newline when previous line ends in static-text",
        "[Lexer]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 abc\nWord 1234567"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cRule, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate_sequence(
            log_parser,
            cInput,
            // NOTE: LogParser will realize "\n1234567" is the start of a new log message
            {{"1234567", "int", {}},
             {" abc", "", {}},
             {"\n", "newLine", {}},
             {"Word", "", {}},
             {" 1234567", "int", {}}}
    );
}

TEST_CASE("Test integer at start of newline when previous line ends in static-text", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 abc\n1234567"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cRule, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate_sequence(
            log_parser,
            cInput,
            {{"1234567", "int", {}}, {" abc", "", {}}, {"\n1234567", "int", {}}}
    );
}

TEST_CASE(
        "Test integer + newline at start of newline when previous line ends in static-text",
        "[Lexer]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 abc\n1234567\n"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cRule, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate_sequence(
            log_parser,
            cInput,
            {{"1234567", "int", {}},
             {" abc", "", {}},
             {"\n1234567", "int", {}},
             {"\n", "newLine", {}}}
    );
}

/** @ingroup LexerTests
 * @brief Verifies that integers are correctly tokenized at the start of a new line
 *        when the previous line ends with a delimiter (space in this case).
 *
 * @details
 * This test ensures the lexer handles newline boundaries correctly and does not
 * incorrectly merge tokens across lines. It focuses on cases where an integer
 * starts immediately after a newline that follows a delimiter.
 *
 * @section rule Rule
 * @code
 * int:\-{0,1}[0-9]+
 * @endcode
 *
 * @section input Input
 * @code
 * 1234567 \n1234567
 * @endcode
 *
 * @section expected Expected Tokens
 * @code
 * "1234567"   -> "int"
 * " "         -> ""
 * "\n1234567" -> "int"
 * @endcode
 *
 * @note
 * This test also checks that leading line breaks do not interfere with token
 * classification, ensuring robustness in log parsing across multiple lines.
 *
 * @test Category: Lexer
 */
TEST_CASE("Test integer at start of newline when previous line ends in a delimiter", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 \n1234567"};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cRule, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate_sequence(
            log_parser,
            cInput,
            {{"1234567", "int", {}}, {" ", "", {}}, {"\n1234567", "int", {}}}
    );
}

/** @ingroup LexerNewlineTests
 * Testing
 */
TEST_CASE("Test capture group repetition and backtracking", "[Lexer]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    string const capture_name{"val"};
    constexpr string_view cVarName{"myVar"};
    constexpr string_view cVarSchema{"myVar:([A-Za-z]+=(?<val>[a-zA-Z0-9]+),){4}"};
    constexpr string_view cTokenString{"userID=123,age=30,height=70,weight=100,"};
    std::pair<std::vector<PrefixTree::position_t>, std::vector<PrefixTree::position_t>> const
            capture_positions{{35, 25, 15, 7}, {37, 27, 17, 10}};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    LogParser log_parser{std::move(schema.release_schema_ast_ptr())};

    CAPTURE(cVarSchema);
    parse_and_validate_sequence(
            log_parser,
            cTokenString,
            {
                {
                    cTokenString, cVarName, {
                        {
                            log_parser.m_lexer.m_symbol_id.at(capture_name), capture_positions
                        }
                    }
                }
            }
    );
    // TODO: add backtracking case
}

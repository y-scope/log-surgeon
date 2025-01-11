#include <codecvt>
#include <locale>
#include <string>
#include <string_view>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

using log_surgeon::SymbolId;
using std::codecvt_utf8;
using std::make_unique;
using std::string;
using std::string_view;
using std::u32string;
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
using log_surgeon::SchemaVarAST;

namespace {
/**
 * Generates an AST for the given `var_schema` string. Then serialize the AST and compare it with
 * the `expected_serialize_ast`.
 * @param var_schema
 * @param expected_serialized_ast
 */
auto test_regex_ast(string_view var_schema, u32string const& expected_serialized_ast) -> void;

/**
 * Converts the characters in a 32-byte unicode string into 4 bytes to generate a 8-byte unicode
 * string.
 * @param u32_str
 * @return The resulting utf8 string.
 */
[[nodiscard]] auto u32string_to_string(u32string const& u32_str) -> string;

auto test_regex_ast(string_view const var_schema, u32string const& expected_serialized_ast)
        -> void {
    log_surgeon::Schema schema;
    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto const* capture_rule_ast = dynamic_cast<SchemaVarAST*>(schema_ast->m_schema_vars[0].get());
    REQUIRE(capture_rule_ast != nullptr);

    auto const actual_string = u32string_to_string(capture_rule_ast->m_regex_ptr->serialize());
    auto const expected_string = u32string_to_string(expected_serialized_ast);
    REQUIRE(actual_string == expected_string);
}

auto u32string_to_string(u32string const& u32_str) -> string {
    wstring_convert<codecvt_utf8<char32_t>, char32_t> converter;
    return converter.to_bytes(u32_str.data(), u32_str.data() + u32_str.size());
}
}  // namespace

TEST_CASE("Test the Schema class", "[Schema]") {
    SECTION("Add a number variable to schema") {
        log_surgeon::Schema schema;
        string const var_name = "myNumber";
        string const var_schema = var_name + string(":") + string("123");
        schema.add_variable(string_view(var_schema), -1);

        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars[0];
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        REQUIRE_NOTHROW([&]() { dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr); }());
    }

    SECTION("Add a capture variable to schema") {
        log_surgeon::Schema schema;
        std::string const var_name = "capture";
        string const var_schema = var_name + string(":") + string("u(?<uID>[0-9]+)");
        schema.add_variable(var_schema, -1);

        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars[0];
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        auto* regex_ast_cat_ptr = dynamic_cast<RegexASTCatByte*>(schema_var_ast.m_regex_ptr.get());
        REQUIRE(nullptr != regex_ast_cat_ptr);
        REQUIRE(nullptr != regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_cat_ptr->get_right());

        auto* regex_ast_literal
                = dynamic_cast<RegexASTLiteralByte const*>(regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_literal);
        REQUIRE('u' == regex_ast_literal->get_character());

        auto* regex_ast_capture
                = dynamic_cast<RegexASTCaptureByte const*>(regex_ast_cat_ptr->get_right());
        REQUIRE(nullptr != regex_ast_capture);
        REQUIRE("uID" == regex_ast_capture->get_group_name());

        auto* regex_ast_multiplication_ast = dynamic_cast<RegexASTMultiplicationByte*>(
                regex_ast_capture->get_group_regex_ast().get()
        );
        REQUIRE(nullptr != regex_ast_multiplication_ast);
        REQUIRE(1 == regex_ast_multiplication_ast->get_min());
        REQUIRE(0 == regex_ast_multiplication_ast->get_max());
        REQUIRE(regex_ast_multiplication_ast->is_infinite());

        auto* regex_ast_group_ast
                = dynamic_cast<RegexASTGroupByte*>(regex_ast_multiplication_ast->get_operand().get()
                );
        REQUIRE(false == regex_ast_group_ast->is_wildcard());
        REQUIRE(1 == regex_ast_group_ast->get_ranges().size());
        REQUIRE('0' == regex_ast_group_ast->get_ranges()[0].first);
        REQUIRE('9' == regex_ast_group_ast->get_ranges()[0].second);
    }

    SECTION("Test AST with tags") {
        // This test validates the serialization of a regex AST with named capture groups. The
        // serialized output includes tags (<n> for positive matches, <~n> for negative matches) to
        // indicate which capture groups are matched or unmatched at each node.
        test_regex_ast(
                // clang-format off
                "capture:"
                "Z|("
                    "A(?<letter>("
                        "(?<letter1>(a)|(b))|"
                        "(?<letter2>(c)|(d))"
                    "))B("
                        "?<containerID>\\d+"
                    ")C"
                ")",
                U"(Z<~letter1><~letter2><~letter><~containerID>)|("
                    "A("
                        "(((a)|(b))<letter1><~letter2>)|"
                        "(((c)|(d))<letter2><~letter1>)"
                    ")<letter>B("
                        "([0-9]){1,inf}"
                    ")<containerID>C"
                ")"
                // clang-format on
        );
    }

    SECTION("Test repetition regex") {
        // Repetition without capture groups untagged and tagged AST are the same
        test_regex_ast("capture:a{0,10}", U"()|((a){1,10})");
        test_regex_ast("capture:a{5,10}", U"(a){5,10}");
        test_regex_ast("capture:a*", U"()|((a){1,inf})");
        test_regex_ast("capture:a+", U"(a){1,inf}");

        // Repetition with capture groups untagged and tagged AST are different
        test_regex_ast("capture:(?<letter>a){0,10}", U"(<~letter>)|(((a)<letter>){1,10})");
        test_regex_ast("capture:(?<letter>a){5,10}", U"((a)<letter>){5,10}");
        test_regex_ast("capture:(?<letter>a)*", U"(<~letter>)|(((a)<letter>){1,inf})");
        test_regex_ast("capture:(?<letter>a)+", U"((a)<letter>){1,inf}");

        // Capture group with repetition
        test_regex_ast("capture:(?<letter>a{0,10})", U"(()|((a){1,10}))<letter>");

        // Complex repetition
        test_regex_ast(
                // clang-format off
                "capture:"
                "("
                    "("
                        "(?<letterA>a)|"
                        "(?<letterB>b)"
                    ")*"
                ")|("
                    "("
                        "(?<letterC>c)|"
                        "(?<letterD>d)"
                    "){0,10}"
                ")",
                U"("
                    U"(<~letterA><~letterB>)|(("
                        U"((a)<letterA><~letterB>)|"
                        U"((b)<letterB><~letterA>)"
                    U"){1,inf})"
                U"<~letterC><~letterD>)|("
                    U"(<~letterC><~letterD>)|(("
                        U"((c)<letterC><~letterD>)|"
                        U"((d)<letterD><~letterC>)"
                    U"){1,10})"
                U"<~letterA><~letterB>)"
                // clang-format on
        );
    }
}

TEST_CASE("Test the Lexer class", "[Lexer]") {
    vector<uint32_t> const cDelimiters{' ', '\n'};
    constexpr string_view cVarName{"myVar"};
    constexpr string_view cVarSchema{"myVar:123"};
    constexpr string_view cTokenString1{"123"};
    constexpr string_view cTokenString2{"234"};

    log_surgeon::Schema schema;
    schema.add_variable(cVarSchema, -1);

    log_surgeon::lexers::ByteLexer lexer;
    lexer.add_delimiters(cDelimiters);

    vector<uint32_t> delimiters;
    for (uint32_t i = 0; i < log_surgeon::cSizeOfByte; i++) {
        if (lexer.is_delimiter(i)) {
            delimiters.push_back(i);
        }
    }

    lexer.m_symbol_id[log_surgeon::cTokenEnd] = (uint32_t)SymbolId::TokenEnd;
    lexer.m_symbol_id[log_surgeon::cTokenUncaughtString] = (uint32_t)SymbolId::TokenUncaughtString;

    lexer.m_id_symbol[(uint32_t)SymbolId::TokenEnd] = log_surgeon::cTokenEnd;
    lexer.m_id_symbol[(uint32_t)SymbolId::TokenUncaughtString] = log_surgeon::cTokenUncaughtString;

    auto const schema_ast{schema.release_schema_ast_ptr()};
    for (auto const& m_schema_var : schema_ast->m_schema_vars) {
        // For log-specific lexing: modify variable regex to contain a delimiter at the start.
        auto delimiter_group{make_unique<RegexASTGroupByte>(RegexASTGroupByte(delimiters))};
        auto* rule = dynamic_cast<SchemaVarAST*>(m_schema_var.get());
        rule->m_regex_ptr = make_unique<RegexASTCatByte>(
                std::move(delimiter_group),
                std::move(rule->m_regex_ptr)
        );
        if (lexer.m_symbol_id.find(rule->m_name) == lexer.m_symbol_id.end()) {
            lexer.m_symbol_id[rule->m_name] = lexer.m_symbol_id.size();
            lexer.m_id_symbol[lexer.m_symbol_id[rule->m_name]] = rule->m_name;
        }
        lexer.add_rule(lexer.m_symbol_id[rule->m_name], std::move(rule->m_regex_ptr));
    }
    lexer.generate();

    log_surgeon::ParserInputBuffer m_input_buffer;
    string token_string{cTokenString1};
    m_input_buffer.set_storage(token_string.data(), token_string.size(), 0, true);
    log_surgeon::Token token1;
    auto error_code{lexer.scan(m_input_buffer, token1)};
    REQUIRE(log_surgeon::ErrorCode::Success == error_code);
    REQUIRE(nullptr != token1.m_type_ids_ptr);
    REQUIRE(1 == token1.m_type_ids_ptr->size());
    REQUIRE(cVarName == lexer.m_id_symbol[token1.m_type_ids_ptr->at(0)]);
    REQUIRE(cTokenString1 == token1.to_string_view());

    error_code = lexer.scan(m_input_buffer, token1);
    REQUIRE(log_surgeon::ErrorCode::Success == error_code);
    REQUIRE(nullptr != token1.m_type_ids_ptr);
    REQUIRE(1 == token1.m_type_ids_ptr->size());
    REQUIRE(log_surgeon::cTokenEnd == lexer.m_id_symbol[token1.m_type_ids_ptr->at(0)]);
    REQUIRE(token1.to_string_view().empty());

    lexer.reset();
    log_surgeon::Token token2;
    token_string = cTokenString2;
    m_input_buffer.set_storage(token_string.data(), token_string.size(), 0, true);
    error_code = lexer.scan(m_input_buffer, token2);
    REQUIRE(log_surgeon::ErrorCode::Success == error_code);
    REQUIRE(nullptr != token2.m_type_ids_ptr);
    REQUIRE(1 == token2.m_type_ids_ptr->size());
    REQUIRE(log_surgeon::cTokenUncaughtString == lexer.m_id_symbol[token2.m_type_ids_ptr->at(0)]);
    REQUIRE(cTokenString2 == token2.to_string_view());

    error_code = lexer.scan(m_input_buffer, token2);
    REQUIRE(log_surgeon::ErrorCode::Success == error_code);
    REQUIRE(nullptr != token2.m_type_ids_ptr);
    REQUIRE(1 == token2.m_type_ids_ptr->size());
    REQUIRE(log_surgeon::cTokenEnd == lexer.m_id_symbol[token2.m_type_ids_ptr->at(0)]);
    REQUIRE(token2.to_string_view().empty());
}

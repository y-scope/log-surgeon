#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>
#include <fmt/core.h>

#include <log_surgeon/BufferParser.hpp>
#include <log_surgeon/Constants.hpp>
#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/LogParser.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/types.hpp>

using log_surgeon::BufferParser;
using log_surgeon::capture_id_t;
using log_surgeon::ErrorCode;
using log_surgeon::finite_automata::PrefixTree;
using log_surgeon::rule_id_t;
using log_surgeon::Schema;
using log_surgeon::SymbolId;
using std::string;
using std::string_view;
using std::unordered_map;
using std::vector;

namespace {
struct CapturePositions {
    std::vector<PrefixTree::position_t> m_start_positions;
    std::vector<PrefixTree::position_t> m_end_positions;
};

struct ExpectedToken {
    std::string_view m_raw_string;
    std::string m_type;
    std::map<string, CapturePositions> m_captures;
};

struct ExpectedEvent {
    std::string_view m_logtype;
    std::string_view m_timestamp_raw;
    std::vector<ExpectedToken> m_tokens;
};

/**
 * Parses the given input and verifies the output is a sequence of tokens matching the expected
 * tokens.
 *
 * If any rule has captures, verifies the captures are in the right place.
 *
 * @param buffer_parser The buffer parser to parse the input with.
 * @param input The input to parse.
 * @param expected_events The expected parsed events.
 */
auto parse_and_validate(
        BufferParser& buffer_parser,
        std::string_view input,
        std::vector<ExpectedEvent> const& expected_events
) -> void;

/**
 * @param map The map to serialize.
 * @return The serialized map.
 */
[[nodiscard]] auto serialize_id_symbol_map(unordered_map<rule_id_t, string> const& map) -> string;

auto parse_and_validate(
        BufferParser& buffer_parser,
        std::string_view input,
        std::vector<ExpectedEvent> const& expected_events
) -> void {
    buffer_parser.reset();

    CAPTURE(serialize_id_symbol_map(buffer_parser.get_log_parser().m_lexer.m_id_symbol));
    CAPTURE(input);
    string input_str(input);

    size_t buffer_offset{0};
    for (auto const& [expected_logtype, expected_timestamp_raw, expected_tokens] : expected_events)
    {
        auto err{buffer_parser
                         .parse_next_event(input_str.data(), input_str.size(), buffer_offset, true)
        };
        REQUIRE(ErrorCode::Success == err);
        auto const& event{buffer_parser.get_log_parser().get_log_event_view()};
        REQUIRE(expected_logtype == event.get_logtype());
        if (nullptr == event.get_timestamp()) {
            REQUIRE(expected_timestamp_raw.empty());
        } else {
            REQUIRE(expected_timestamp_raw == event.get_timestamp()->to_string());
        }

        uint32_t event_offset{0};
        if (nullptr == event.get_timestamp()) {
            event_offset = 1;
        }

        REQUIRE(expected_tokens.size() == event.get_log_output_buffer()->pos() - event_offset);
        for (size_t i{0}; i < expected_tokens.size(); ++i) {
            auto const& [expected_raw_string, expected_type, expected_captures]{expected_tokens[i]};
            auto token{event.get_log_output_buffer()->get_token(i + event_offset)};
            CAPTURE(i);
            REQUIRE(expected_raw_string == token.to_string());

            uint32_t expected_token_type;
            if (expected_type.empty()) {
                expected_token_type = static_cast<uint32_t>(SymbolId::TokenUncaughtString);
            } else {
                CAPTURE(expected_type);
                REQUIRE(buffer_parser.get_log_parser().get_symbol_id(expected_type).has_value());
                expected_token_type
                        = buffer_parser.get_log_parser().get_symbol_id(expected_type).value();
            }
            auto const token_type{token.m_type_ids_ptr->at(0)};
            REQUIRE(expected_token_type == token_type);

            if (false == expected_captures.empty()) {
                auto const& lexer{buffer_parser.get_log_parser().m_lexer};
                auto optional_capture_ids{lexer.get_capture_ids_from_rule_id(token_type)};
                REQUIRE(optional_capture_ids.has_value());

                if (false == optional_capture_ids.has_value()) {
                    return;
                }

                for (auto const capture_id : optional_capture_ids.value()) {
                    auto const capture_name{lexer.m_id_symbol.at(capture_id)};
                    REQUIRE(expected_captures.contains(capture_name));
                    auto optional_reg_ids{lexer.get_reg_ids_from_capture_id(capture_id)};
                    REQUIRE(optional_reg_ids.has_value());
                    if (false == optional_reg_ids.has_value()) {
                        return;
                    }
                    auto const [start_reg_id, end_reg_id]{optional_reg_ids.value()};
                    auto const actual_start_positions{token.get_reversed_reg_positions(start_reg_id)
                    };
                    auto const actual_end_positions{token.get_reversed_reg_positions(end_reg_id)};
                    auto const [expected_start_positions, expected_end_positions]{
                            expected_captures.at(capture_name)
                    };
                    REQUIRE(expected_start_positions == actual_start_positions);
                    REQUIRE(expected_end_positions == actual_end_positions);
                }
            }
        }
    }
    REQUIRE(buffer_parser.done());
}

auto serialize_id_symbol_map(unordered_map<rule_id_t, string> const& map) -> string {
    string serialized_map;
    for (auto const& [id, symbol] : map) {
        serialized_map += fmt::format("{}->{},", id, symbol);
    }
    return serialized_map;
}

}  // namespace

/**
 * @ingroup test_buffer_parser_no_capture
 *
 * @brief Tests the buffer parser behavior when parsing variables without capture groups.
 *
 * @details
 * This test verifies that the buffer parser correctly matches exact variable patterns when
 * no capture groups are involved. It confirms the `BUfferParser`:
 * - Recognizes a variable exactly matching the defined schema ("myVar:userID=123").
 * - Treats close but non-matching strings as uncaught tokens.
 * - Correctly classifies tokens that don't match any variable schema as uncaught strings.
 *
 * @section schema Schema Definition
 * @code
 * delimiters: \n\r\[:,)
 * myVar:userID=123
 * @endcode
 *
 * @section input Test Input
 * @code
 * "userID=123 userID=234 userID=123 123 userID=123"
 * @endcode
 *
 * @section expected Expected Logtype
 * @code
 * "<myVar> userID=234 <myVar> 123 <myVar>"
 * @endcode
 *
 * @section expected Expected Tokenization
 * @code
 * "userID=123" -> "myVar"
 * " userID=234" -> uncaught string
 * " userID=123" -> "myVar"
 * " 123" -> uncaught string
 * " userID=123" -> "myVar"
 * @endcode
 */
TEST_CASE("Test buffer parser without capture groups", "[BufferParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{"myVar:userID=123"};
    constexpr string_view cInput{"userID=123 userID=234 userID=123 123 userID=123"};
    ExpectedEvent const expected_event{
            .m_logtype{R"(<myVar> userID=234 <myVar> 123 <myVar>)"},
            .m_timestamp_raw{""},
            .m_tokens{
                    {{"userID=123", "myVar", {}},
                     {" userID=234", "", {}},
                     {" userID=123", "myVar", {}},
                     {" 123", "", {}},
                     {" userID=123", "myVar", {}}}
            }
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser(std::move(schema.release_schema_ast_ptr()));

    parse_and_validate(buffer_parser, cInput, {expected_event});
}

/**
 * @ingroup test_buffer_parser_newline_vars
 *
 * @brief Test variable after static-text at the start of a newline when previous line ends in a
 * variable.
 *
 * @details
 * This test verifies that when a line ends with a variable token and the next line starts with
 * static text followed by an integer variable, the `BufferParser` correctly recognizes the newline
 * as a delimiter and parses the tokens appropriately.
 *
 * @section schema Schema Definition
 * @code
 * delimiters: \n\r\[:,)
 * int: \-{0,1}[0-9]+
 * @endcode
 *
 * @section input Test Input
 * @code
 * "1234567\nText 1234567"
 * @endcode
 *
 * @section expected Expected Logtype
 * @code
 * "<int><newLine>"
 * "Text <int>"
 * @endcode
 *
 * @section expected Expected Tokenization
 * @code
 * "1234567" -> "int"
 * "\n" -> "newLine"
 * "Text" -> uncaught string
 * " 1234567" -> "int"
 * @endcode
 */
TEST_CASE("Test buffer parser first token after newline #1", "[BufferParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567\nText 1234567"};
    ExpectedEvent const expected_event1{
            .m_logtype{R"(<int><newLine>)"},
            .m_timestamp_raw{""},
            .m_tokens{{{"1234567", "int", {}}, {"\n", "newLine", {}}}}
    };
    ExpectedEvent const expected_event2{
            .m_logtype{R"(Text <int>)"},
            .m_timestamp_raw{""},
            .m_tokens{{{"Text", "", {}}, {" 1234567", "int", {}}}}
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2});
}

/**
 * @ingroup test_buffer_parser_newline_vars
 *
 * @brief Test variable after static-text at start of newline when previous line ends in
 * static-text.
 *
 * @details
 * This test verifies that when a line ends with static text and the next line starts with static
 * text followed by an integer variable, the `BufferParser` identifies the newline properly and
 * tokenizes the input correctly.
 *
 * @section schema Schema Definition
 * @code
 * delimiters: \n\r\[:,)
 * int: \-{0,1}[0-9]+
 * @endcode
 *
 * @section input Test Input
 * @code
 * "1234567 abc\nText 1234567"
 * @endcode
 *
 * @section expected Expected Logtype
 * @code
 * "<int> abc<newLine>"
 * "Text <int>"
 * @endcode
 *
 * @section expected Expected Tokenization
 * @code
 * "1234567" -> "int"
 * " abc" -> uncaught string
 * "\n" -> "newLine"
 * "Text" -> uncaught string
 * " 1234567" -> "int"
 * @endcode
 */
TEST_CASE("Test buffer parser first token after newline #2", "[BufferParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 abc\nText 1234567"};
    ExpectedEvent const expected_event1{
            .m_logtype{R"(<int> abc<newLine>)"},
            .m_timestamp_raw{""},
            .m_tokens{{{"1234567", "int", {}}, {" abc", "", {}}, {"\n", "newLine", {}}}}
    };
    ExpectedEvent const expected_event2{
            .m_logtype{R"(Text <int>)"},
            .m_timestamp_raw{""},
            .m_tokens{{{"Text", "", {}}, {" 1234567", "int", {}}}}
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2});
}

/**
 * @ingroup test_buffer_parser_newline_vars
 *
 * @brief Test variable at start of newline when previous line ends in static-text.
 *
 * @details
 * This test verifies that when a line ends with static text and the next line starts directly with
 * an integer variable, the `BufferParser` treats the newline and variable token correctly.
 *
 * @section schema Schema Definition
 * @code
 * delimiters: \n\r\[:,)
 * int: \-{0,1}[0-9]+
 * @endcode
 *
 * @section input Test Input
 * @code
 * "1234567 abc\n1234567"
 * @endcode
 *
 * @section expected Expected Logtype
 * @code
 * "<int> abc\n"
 * "<int>"
 * @endcode
 *
 * @section expected Expected Tokenization
 * @code
 * "1234567" -> "int"
 * " abc" -> uncaught string
 * "\n" -> uncaught string
 * "1234567" -> "int"
 * @endcode
 */
TEST_CASE("Test buffer parser first token after newline #3", "[BufferParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 abc\n1234567"};
    ExpectedEvent const expected_event1{
            .m_logtype{"<int> abc\n"},
            .m_timestamp_raw{""},
            .m_tokens{{{"1234567", "int", {}}, {" abc", "", {}}, {"\n", "", {}}}}
    };
    ExpectedEvent const expected_event2{
            .m_logtype{R"(<int>)"},
            .m_timestamp_raw{""},
            .m_tokens{{{"1234567", "int", {}}}}
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2});
}

/**
 * @ingroup test_buffer_parser_newline_vars
 *
 * @brief Test variable followed by newline at start of newline when previous line ends in
 * static-text.
 *
 * @details
 * This test verifies that when a line ends with static text, and the next line contains an integer
 * variable followed by a newline, the `BufferParser` correctly separates the tokens, recognizing
 * the newline delimiter.
 *
 * @section schema Schema Definition
 * @code
 * delimiters: \n\r\[:,)
 * int: \-{0,1}[0-9]+
 * @endcode
 *
 * @section input Test Input
 * @code
 * "1234567 abc\n1234567\n"
 * @endcode
 *
 * @section expected Expected Logtype
 * @code
 * "<int> abc\n"
 * "<int><newLine>"
 * @endcode
 *
 * @section expected Expected Tokenization
 * @code
 * "1234567" -> "int"
 * " abc" -> uncaught string
 * "\n" -> uncaught string
 * "1234567" -> "int"
 * "\n" -> "newLine"
 * @endcode
 */
TEST_CASE("Test buffer parser first token after newline #4", "[BufferParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cInput{"1234567 abc\n1234567\n"};
    ExpectedEvent const expected_event1{
            .m_logtype{"<int> abc\n"},
            .m_timestamp_raw{""},
            .m_tokens{{{"1234567", "int", {}}, {" abc", "", {}}, {"\n", "", {}}}}
    };
    ExpectedEvent const expected_event2{
            .m_logtype{R"(<int><newLine>)"},
            .m_timestamp_raw{""},
            .m_tokens{{{"1234567", "int", {}}, {"\n", "newLine", {}}}}
    };
    ExpectedEvent const expected_event3{.m_logtype{""}, .m_timestamp_raw{""}, .m_tokens{}};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2, expected_event3});
}

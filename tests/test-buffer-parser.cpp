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

TEST_CASE("Test log parser without capture groups", "[LogParser]") {
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

TEST_CASE("Test log parser with capture groups", "[LogParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{"myVar:userID=(?<uid>123)"};
    constexpr string_view cInput{"userID=123 userID=234 userID=123 123 userID=123"};

    ExpectedEvent const expected_event{
            .m_logtype{R"(userID=<uid> userID=234  userID=<uid> 123  userID=<uid>)"},
            .m_timestamp_raw{""},
            .m_tokens{
                    {{"userID=123", "myVar", {{{"uid", {{7}, {10}}}}}},
                     {" userID=234", "", {}},
                     {" userID=123", "myVar", {{{"uid", {{29}, {32}}}}}},
                     {" 123", "", {}},
                     {" userID=123", "myVar", {{{"uid", {{44}, {47}}}}}}}
            }
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser(std::move(schema.release_schema_ast_ptr()));

    parse_and_validate(buffer_parser, cInput, {expected_event});
}

TEST_CASE("Test log parser with CLP default schema", "[LogParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema1{
            R"(timestamp:[0-9]{4}\-[0-9]{2}\-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}[,\.][0-9]{0,3})"
    };
    constexpr string_view cVarSchema2{R"(int:\-{0,1}[0-9]+)"};
    constexpr string_view cVarSchema3{R"(float:\-{0,1}[0-9]+\.[0-9]+)"};
    constexpr string_view cVarSchema4{R"(hex:[a-fA-F]+)"};
    constexpr string_view cVarSchema5{
            R"(keyValuePair:[^ \r\n=]+=(?<val>[^ \r\n]*[A-Za-z0-9][^ \r\n]*))"
    };
    constexpr string_view cVarSchema6{R"(hasNumber:={0,1}[^ \r\n=]*\d[^ \r\n=]*={0,1})"};
    constexpr string_view cInput{"2012-12-12 12:12:12.123 123 123.123 abc userID=123 text user123"};
    ExpectedEvent const expected_event{
            .m_logtype{R"( <int> <float> <hex>  userID=<val> text <hasNumber>)"},
            .m_timestamp_raw{"2012-12-12 12:12:12.123"},
            .m_tokens{
                    {{"2012-12-12 12:12:12.123", "firstTimestamp", {}},
                     {" 123", "int", {}},
                     {" 123.123", "float", {}},
                     {" abc", "hex", {}},
                     {" userID=123", "keyValuePair", {{{"val", {{47}, {50}}}}}},
                     {" text", "", {}},
                     {" user123", "hasNumber", {}}}
            }
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema1, -1);
    schema.add_variable(cVarSchema2, -1);
    schema.add_variable(cVarSchema3, -1);
    schema.add_variable(cVarSchema4, -1);
    schema.add_variable(cVarSchema5, -1);
    schema.add_variable(cVarSchema6, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event});
}

TEST_CASE(
        "Test integer after static-text at start of newline when previous line ends in a variable",
        "[LogParser]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
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
    schema.add_variable(cRule, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2});
}

TEST_CASE(
        "Test integer after static-text at start of newline when previous line ends in static-text",
        "[LogParser]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
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
    schema.add_variable(cRule, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2});
}

TEST_CASE(
        "Test integer at start of newline when previous line ends in static-text",
        "[LogParser]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
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
    schema.add_variable(cRule, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2});
}

TEST_CASE(
        "Test integer + newline at start of newline when previous line ends in static-text",
        "[LogParser]"
) {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cRule{R"(int:\-{0,1}[0-9]+)"};
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
    schema.add_variable(cRule, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event1, expected_event2, expected_event3});
}

// TODO: fix this case:
/*
TEST_CASE("Test capture group repetition and backtracking", "[LogParser]") {
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema{"keyValuePair:([A-Za-z]+=(?<val>[a-zA-Z0-9]+),){4}"};
    constexpr string_view cInput{"userID=123,age=30,height=70,weight=100,"};
    ExpectedEvent const expected_event{
            .m_logtype{R"(userID=<val>,age=<val>,height=<val>,weight=<val>,)"},
            .m_timestamp_raw{""},
            .m_tokens{
                    {{"userID=123,age=30,height=70,weight=100,",
                      "keyValuePair",
                      {{{"val", {{35, 25, 15, 7}, {37, 27, 17, 10}}}}}}}
            }
    };

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema, -1);
    BufferParser buffer_parser{std::move(schema.release_schema_ast_ptr())};

    parse_and_validate(buffer_parser, cInput, {expected_event});
    // TODO: add backtracking case
}
*/

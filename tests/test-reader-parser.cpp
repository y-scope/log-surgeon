#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/LogParser.hpp>
#include <log_surgeon/Reader.hpp>
#include <log_surgeon/ReaderParser.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/types.hpp>

#include <catch2/catch_test_macros.hpp>
#include <fmt/format.h>

using log_surgeon::capture_id_t;
using log_surgeon::cStaticByteBuffSize;
using log_surgeon::ErrorCode;
using log_surgeon::finite_automata::PrefixTree;
using log_surgeon::Reader;
using log_surgeon::ReaderParser;
using log_surgeon::rule_id_t;
using log_surgeon::Schema;
using log_surgeon::SymbolId;
using log_surgeon::Token;
using std::map;
using std::string;
using std::string_view;
using std::unordered_map;
using std::vector;

namespace {
struct CapturePositions {
    vector<PrefixTree::position_t> m_start_positions;
    vector<PrefixTree::position_t> m_end_positions;
};

struct ExpectedToken {
    string_view m_raw_string;
    string m_type;
    map<string, CapturePositions> m_captures;
};

struct ExpectedEvent {
    string_view m_logtype;
    string_view m_timestamp_raw;
    vector<ExpectedToken> m_tokens;
};

/**
 * Parses the given input and verifies the output is a sequence of tokens matching the expected
 * tokens.
 *
 * If any rule has captures, verifies the captures are in the right place.
 *
 * @param reader_parser The reader parser to parse the input with.
 * @param input The input to parse.
 * @param expected_events The expected parsed events.
 */
auto parse_and_validate(
        ReaderParser& reader_parser,
        string_view input,
        vector<ExpectedEvent> const& expected_events
) -> void;

/**
 * @param map The map to serialize.
 * @return The serialized map.
 */
[[nodiscard]] auto serialize_id_symbol_map(unordered_map<rule_id_t, string> const& map) -> string;

auto parse_and_validate(
        ReaderParser& reader_parser,
        string_view input,
        vector<ExpectedEvent> const& expected_events
) -> void {
    size_t curr_pos{0};

    Reader reader{[&](char* buffer, size_t const count, size_t& read_to) -> ErrorCode {
        if (input.size() <= curr_pos) {
            read_to = 0;
            return ErrorCode::EndOfFile;
        }

        read_to = input.size() - curr_pos;
        if(read_to  > count) {
            read_to = count;
        }

        std::memcpy(buffer, input.data() + curr_pos, read_to);
        curr_pos += read_to;
        return ErrorCode::Success;
    }};


    reader_parser.reset_and_set_reader(reader);

    CAPTURE(serialize_id_symbol_map(reader_parser.get_log_parser().m_lexer.m_id_symbol));
    CAPTURE(input);
    string input_str(input);

    size_t count{0};
    for (auto const& [expected_logtype, expected_timestamp_raw, expected_tokens] : expected_events)
    {
        CAPTURE(count);
        count++;

        auto err{reader_parser.parse_next_event()};
        REQUIRE(ErrorCode::Success == err);
        auto const& event{reader_parser.get_log_parser().get_log_event_view()};
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
                REQUIRE(reader_parser.get_log_parser().get_symbol_id(expected_type).has_value());
                expected_token_type
                        = reader_parser.get_log_parser().get_symbol_id(expected_type).value();
            }
            auto const token_type{token.get_type_ids()->at(0)};
            REQUIRE(expected_token_type == token_type);

            if (false == expected_captures.empty()) {
                auto const& lexer{reader_parser.get_log_parser().m_lexer};
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
                    auto const actual_start_positions{
                            token.get_reversed_reg_positions(start_reg_id)
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
    REQUIRE(reader_parser.done());
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
 * @defgroup test_reader_parser_no_capture Reader parser using variables without capture groups.
 * @brief Tests covering variable matching without regex capture groups.
 */

/**
 * @ingroup test_reader_parser_no_capture
 * @brief Tests the reader parser behavior when parsing variables without capture groups.
 *
 * This test verifies that the reader parser correctly matches exact variable patterns when no
 * capture groups are involved. It confirms the `ReaderParser`:
 * - Recognizes a variable exactly matching the defined schema ("myVar:userID=123").
 * - Treats close but non-matching strings as uncaught tokens.
 * - Correctly classifies tokens that don't match any variable schema as uncaught strings.
 *
 * ### Schema Definition
 * @code
 * delimiters: \n\r\[:,
 * myVar:userID=123
 * @endcode
 *
 * ### Test Input
 * @code
 * "userID=123 userID=234 userID=123 123 userID=123"
 * @endcode
 *
 * ### Expected Logtype
 * @code
 * "<myVar> userID=234 <myVar> 123 <myVar>"
 * @endcode
 *
 * ### Expected Tokenization
 * @code
 * "userID=123" -> "myVar"
 * " userID=234" -> uncaught string
 * " userID=123" -> "myVar"
 * " 123" -> uncaught string
 * " userID=123" -> "myVar"
 * @endcode
 */
TEST_CASE("single_line_without_capture_reader_parser", "[ReaderParser]") {
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
    ReaderParser reader_parser(std::move(schema.release_schema_ast_ptr()));

    parse_and_validate(reader_parser, cInput, {expected_event});
}

/**
 * @defgroup unit_tests_reader_parser_wrap_around `ReaderParser` unit tests.
 * @brief Unit tests for `ReaderParser` wrap around handling.

 * These unit tests contain the `ReaderParser` tag.
 */

/**
 * @ingroup unit_tests_reader_parser_wrap_around
 * @brief Tests the reader parser behavior when parsing variables without capture groups.
 *
 * This test verifies that the reader parser correctly handles the wrap around handling when a log
 * begins or ends near the boundaries of the buffer:
 * - Considers the case where the log ends right at the end of the buffer.
 * - Considers the case where the log starts right after wrapping around.
 * - Considers every case in between, which has the added benefit of testing every case for each
 *   tested variable as well (which include a capture).
 */
TEST_CASE("reader_parser_wrap_around", "[ReaderParser]") {
    REQUIRE(48000 == cStaticByteBuffSize);
    
    constexpr string_view cDelimitersSchema{R"(delimiters: \n\r\[:,)"};
    constexpr string_view cVarSchema1{"myVar:userID=123"};
    constexpr string_view cVarSchema2{"myCapture:userID=(?<capture>234)"};
    constexpr string_view cInput1{"userID=123 userID=234 userID=123 123 userID=123\n"};
    constexpr string_view cInput3{"userID=123 userID=234 userID=123 123 userID=123"};
    constexpr uint32_t cNumInput1{998};
    constexpr uint32_t cRemainingSpace{cStaticByteBuffSize - cInput1.size() * cNumInput1};

    Schema schema;
    schema.add_delimiters(cDelimitersSchema);
    schema.add_variable(cVarSchema1, -1);
    schema.add_variable(cVarSchema2, -1);
    ReaderParser reader_parser(std::move(schema.release_schema_ast_ptr()));

    for (int32_t offset{cInput3.size()}; offset >= 0; --offset) {
        CAPTURE(offset);

        string user_var{"userID=123"};
        string remaining_filler(cRemainingSpace - user_var.size() - offset - 2, 'a');
        string input2{user_var + " " + remaining_filler + "\n"};
        string logtype2{"<myVar> " + remaining_filler + "\n"};

        string cInput;
        for (uint32_t i{0}; i < cNumInput1; i++) {
            cInput += cInput1;
        }
        REQUIRE(cInput.size() == cStaticByteBuffSize - cRemainingSpace);
        cInput += input2;
        REQUIRE(cInput.size() == cStaticByteBuffSize - offset);
        cInput += cInput3;

        ExpectedEvent expected_event1{
            .m_logtype{"<myVar> userID=<capture> <myVar> 123 <myVar>\n"},
            .m_timestamp_raw{""},
            .m_tokens{
                        {{"userID=123", "myVar", {}},
                         {" userID=234", "myCapture", {{{"capture",{{18}, {21}}}}}},
                         {" userID=123", "myVar", {}},
                         {" 123", "", {}},
                         {" userID=123", "myVar", {}},
                         {"\n", "", {}}}
            }
        };

        string_view logtype2_view{logtype2};
        string_view user_var_view{user_var};
        string remaining_filler_with_space{" " + remaining_filler};
        string_view remaining_filler_view{remaining_filler_with_space};
        ExpectedEvent expected_event2{
            .m_logtype{logtype2_view},
            .m_timestamp_raw{""},
            .m_tokens{
                    {{user_var_view, "myVar", {}},
                     {remaining_filler_view, "", {}},
                     {"\n", "", {}}}
            }
        };

        int32_t log_start_pos{static_cast<int32_t>(cStaticByteBuffSize) - offset};
        int32_t cap_begin{log_start_pos+18};
        if(cap_begin >= cStaticByteBuffSize) {
            cap_begin -= cStaticByteBuffSize;
        }
        int32_t cap_end{log_start_pos+21};
        if(cap_end >= cStaticByteBuffSize) {
            cap_end -= cStaticByteBuffSize;
        }
        ExpectedEvent expected_event3{
            .m_logtype{"<myVar> userID=<capture> <myVar> 123 <myVar>"},
            .m_timestamp_raw{""},
            .m_tokens{
                    {{"userID=123", "myVar", {}},
                     {" userID=234", "myCapture", {{{"capture",{{cap_begin}, {cap_end}}}}}},
                     {" userID=123", "myVar", {}},
                     {" 123", "", {}},
                     {" userID=123", "myVar", {}}}
            }
        };
        
        vector<ExpectedEvent> expected_events;
        for (uint32_t i{0}; i < cNumInput1; ++i) {
            expected_events.push_back(expected_event1);
            auto& capture{expected_event1.m_tokens.at(1).m_captures["capture"]};
            capture.m_start_positions.at(0) += cInput1.size();
            capture.m_end_positions.at(0) += cInput1.size();
        }
        expected_events.push_back(expected_event2);
        expected_events.push_back(expected_event3);

        parse_and_validate(reader_parser, cInput, expected_events);
    }
}

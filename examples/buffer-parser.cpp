#include <algorithm>
#include <fstream>
#include <iostream>
#include <optional>
#include <string>
#include <vector>

#include <log_surgeon/BufferParser.hpp>
#include <log_surgeon/Constants.hpp>
#include <log_surgeon/LogEvent.hpp>

#include "common.hpp"

using namespace std;
using namespace log_surgeon;

auto process_logs(string& schema_path, string const& input_path) -> void {
    BufferParser parser{schema_path};
    optional<uint32_t> loglevel_id{parser.get_variable_id("loglevel")};
    if (false == loglevel_id.has_value()) {
        throw runtime_error("No 'loglevel' in schema.");
    }

    ifstream infs{input_path, ios::binary | ios::in};
    if (!infs.is_open()) {
        cout << "Failed to open: " << input_path << endl;
        return;
    }

    constexpr ssize_t const cSize{4096L * 8};  // 8 pages
    vector<char> buf(cSize);
    infs.read(buf.data(), cSize);
    ssize_t valid_size{infs.gcount()};
    bool input_done{false};
    if (infs.eof()) {
        input_done = true;
    }
    parser.reset();

    cout << "# Parsing timestamp and loglevel for each log event in " << input_path << ":" << endl;

    vector<LogEvent> multiline_logs;
    size_t offset{0};
    while (false == parser.done()) {
        if (ErrorCode err{parser.parse_next_event(buf.data(), valid_size, offset, input_done)};
            ErrorCode::Success != err)
        {
            // The only expected error is the parser has read to the bound
            // of the buffer.
            if (ErrorCode::BufferOutOfBounds != err) {
                throw runtime_error("Parsing Failed.");
            }
            if (input_done) {
                break;
            }

            // If the offset is 0 the parser has not found the end of the
            // log event in the entire buffer, so we make it larger.
            // Otherwise, we move the remaing valid data to the start of
            // the buffer.
            if (0 == offset) {
                buf.resize(buf.size() * 2);
            } else {
                move(buf.begin() + offset, buf.begin() + valid_size, buf.begin());
                valid_size -= offset;
                offset = 0;
            }

            infs.read(&*(buf.begin() + valid_size), buf.size() - valid_size);
            ssize_t read{infs.gcount()};
            if (infs.eof()) {
                input_done = true;
            }
            valid_size += read;
            continue;
        }

        LogEventView const& event = parser.get_log_parser().get_log_event_view();
        cout << "log: " << event.to_string() << endl;
        print_timestamp_loglevel(event, *loglevel_id);
        cout << "logtype: " << event.get_logtype() << endl;
        if (event.is_multiline()) {
            multiline_logs.emplace_back(event);
        }
    }

    cout << endl << "# Printing multiline logs:" << endl;
    for (auto const& log : multiline_logs) {
        cout << log.to_string() << endl;
    }
}

auto main(int argc, char* argv[]) -> int {
    if (int err{check_input(argc, argv)}; 0 != err) {
        return err;
    }
    string schema_path{argv[1]};
    string input_path{argv[2]};
    process_logs(schema_path, input_path);
    return 0;
}

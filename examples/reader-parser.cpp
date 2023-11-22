#include <fstream>
#include <iostream>
#include <optional>
#include <string>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/ReaderParser.hpp>

#include "common.hpp"

using namespace std;
using namespace log_surgeon;

auto process_logs(string& schema_path, string const& input_path) -> void {
    ReaderParser parser{schema_path};
    optional<uint32_t> loglevel_id{parser.get_variable_id("loglevel")};
    if (false == loglevel_id.has_value()) {
        throw runtime_error("No 'loglevel' in schema.");
    }

    ifstream infs{input_path, ios::binary | ios::in};
    if (!infs.is_open()) {
        cout << "Failed to open: " << input_path << endl;
        return;
    }

    Reader reader{[&](char* buf, size_t count, size_t& read_to) -> ErrorCode {
        infs.read(buf, count);
        read_to = infs.gcount();
        if (0 == read_to && infs.eof()) {
            return ErrorCode::EndOfFile;
        }
        return ErrorCode::Success;
    }};
    parser.reset_and_set_reader(reader);

    cout << "# Parsing timestamp and loglevel for each log event in " << input_path << ":" << endl;

    vector<LogEvent> multiline_logs;
    while (false == parser.done()) {
        if (ErrorCode err{parser.parse_next_event()}; ErrorCode::Success != err) {
            throw runtime_error("Parsing Failed");
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

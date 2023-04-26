#include <filesystem>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include <log_surgeon/LogEvent.hpp>

#include "common.hpp"

using namespace std;
using namespace log_surgeon;

auto check_input(int argc, char* argv[]) -> int {
    int ret{0};
    if (3 != argc) {
        ret = 1;
        cout << "Not enough arguments." << endl;
    } else if (filesystem::path f{argv[1]}; false == filesystem::exists(f)) {
        ret = 2;
        cout << "Schema file does not exist." << endl;
    } else if (filesystem::path f{argv[2]}; false == filesystem::exists(f)) {
        ret = 3;
        cout << "Input file does not exist." << endl;
    }
    if (0 != ret) {
        cout << "usage: <path to schema file> <path to input log file>" << endl;
    }
    return ret;
}

auto print_timestamp_loglevel(LogEventView const& event, uint32_t loglevel_id) -> void {
    Token* timestamp{event.get_timestamp()};
    Token* loglevel{nullptr};
    if (nullptr != timestamp) {
        if (auto const& vec{event.get_variables(loglevel_id)}; false == vec.empty()) {
            loglevel = vec[0];
        }
    }
    if (nullptr != timestamp) {
        cout << "timestamp: ";
        cout << timestamp->to_string_view();
    }
    if (nullptr != loglevel) {
        cout << ", loglevel:";
        cout << loglevel->to_string_view();
    }
    cout << endl;
}

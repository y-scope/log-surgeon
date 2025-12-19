#include "common.hpp"

#include <cstdint>
#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/Token.hpp>

namespace examples {
using std::cout;

auto check_input(std::vector<std::string> const& args) -> int {
    int ret{0};
    if (2 != args.size()) {
        ret = 1;
        cout << "Not enough arguments.\n";
    } else if (std::filesystem::path const f{args[0]}; false == std::filesystem::exists(f)) {
        ret = 2;
        cout << "Schema file does not exist.\n";
    } else if (std::filesystem::path const f{args[1]}; false == std::filesystem::exists(f)) {
        ret = 3;
        cout << "Input file does not exist.\n";
    }
    if (0 != ret) {
        cout << "usage: <path to schema file> <path to input log file>\n";
    }
    return ret;
}

auto print_timestamp_loglevel(log_surgeon::LogEventView const& event, uint32_t loglevel_id)
        -> void {
    log_surgeon::Token* timestamp{event.get_timestamp()};
    log_surgeon::Token* loglevel{nullptr};
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
    cout << "\n";
}
}  // namespace examples

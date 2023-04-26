#ifndef COMMON_HPP
#define COMMON_HPP

#include <string>
#include <vector>

#include <log_surgeon/LogEvent.hpp>

auto check_input(int argc, char* argv[]) -> int;

auto print_timestamp_loglevel(log_surgeon::LogEventView const& event, uint32_t loglevel_id) -> void;

#endif // COMMON_HPP

#ifndef EXAMPLES_COMMON_HPP
#define EXAMPLES_COMMON_HPP

#include <cstdint>
#include <string>
#include <vector>

#include <log_surgeon/LogEvent.hpp>

namespace examples {
auto check_input(std::vector<std::string> const& args) -> int;

auto print_timestamp_loglevel(log_surgeon::LogEventView const& event, uint32_t loglevel_id) -> void;
}  // namespace examples

#endif  // EXAMPLES_COMMON_HPP

#ifndef LOG_SURGEON_UTILS_HPP
#define LOG_SURGEON_UTILS_HPP

#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

template <typename... Args>
auto strfmt(std::string const& fmt, Args... args) -> std::string {
    int size = std::snprintf(nullptr, 0, fmt.c_str(), args...);
    if (size <= 0) {
        throw std::runtime_error("Error during formatting.");
    }
    // Add 1 for null character to terminate the C string
    std::vector<char> buf(size + 1);
    size = std::snprintf(buf.data(), buf.size(), fmt.c_str(), args...);
    if (size <= 0) {
        throw std::runtime_error("Error during formatting.");
    }
    return {buf.data(), buf.data() + size};
}

#endif  // LOG_SURGEON_UTILS_HPP

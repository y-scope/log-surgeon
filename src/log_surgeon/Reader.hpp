#ifndef LOG_SURGEON_LIBRARY_READER_HPP
#define LOG_SURGEON_LIBRARY_READER_HPP

#include <functional>

#include <log_surgeon/Constants.hpp>

namespace log_surgeon {
/**
 * Minimal interface necessary for the parser to invoke reading as necessary.
 * Allowing the parser to invoke read helps users avoid unnecessary copying,
 * makes the lifetime of LogEventViews easier to understand, and makes the user
 * code cleaner.
 */
class Reader {
public:
    /**
     * Function to read from some unknown source into specified destination
     * buffer.
     * @param std::char* Destination byte buffer to read into
     * @param size_t Amount to read up to
     * @param size_t& The amount read
     * @return ErrorCode::EndOfFile if the end of file was reached
     * @return ErrorCode::Success if any bytes read
     */
    std::function<ErrorCode(char*, size_t, size_t&)> read{};
};

}  // namespace log_surgeon

#endif  // LOG_SURGEON_LIBRARY_READER_HPP

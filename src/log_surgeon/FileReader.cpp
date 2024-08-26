#include "FileReader.hpp"

#include <cassert>
#include <cerrno>
#include <cstdint>
#include <ios>
#include <string>
#include <variant>

#include <log_surgeon/Constants.hpp>

using std::string;
using std::variant;

namespace log_surgeon {
auto FileReader::read(char* buf, uint32_t const num_bytes_to_read, uint32_t& num_bytes_read)
        -> ErrorCode {
    if (false == m_file_stream.is_open()) {
        return ErrorCode::NotInit;
    }
    if (nullptr == buf) {
        return ErrorCode::BadParam;
    }

    m_file_stream.read(buf, num_bytes_to_read);
    num_bytes_read = m_file_stream.gcount();

    if (num_bytes_read < num_bytes_to_read) {
        if (m_file_stream.eof()) {
            return ErrorCode::EndOfFile;
        }
        if (m_file_stream.fail()) {
            return ErrorCode::Errno;
        }
    }
    return ErrorCode::Success;
}

auto FileReader::open(string const& path) -> ErrorCode {
    // Cleanup in case caller forgot to call close before calling this function
    if (auto const err = close(); ErrorCode::Success != err) {
        return err;
    }

    m_file_stream.open(path, std::ios::binary);
    if (false == m_file_stream.is_open()) {
        if (ENOENT == errno) {
            return ErrorCode::FileNotFound;
        }
        return ErrorCode::Errno;
    }
    return ErrorCode::Success;
}

auto FileReader::close() -> ErrorCode {
    if (m_file_stream.is_open()) {
        m_file_stream.close();
        if (m_file_stream.fail()) {
            return ErrorCode::Errno;
        }
    }
    return ErrorCode::Success;
}

auto FileReader::open_and_read_to_line_number(
        std::string const& schema_path,
        uint32_t const line_num
) -> variant<string, ErrorCode> {
    if (ErrorCode error_code = open(schema_path); ErrorCode::Success != error_code) {
        return error_code;
    }
    std::string line;
    for (uint32_t current_line = 0; current_line <= line_num; current_line++) {
        std::getline(m_file_stream, line);
        if (m_file_stream.eof()) {
            return ErrorCode::EndOfFile;
        }
        if (m_file_stream.fail()) {
            return ErrorCode::Errno;
        }
    }
    return line;
}
}  // namespace log_surgeon

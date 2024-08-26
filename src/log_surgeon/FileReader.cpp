#include "FileReader.hpp"

#include <cassert>
#include <cerrno>
#include <cstdio>
#include <string>

#include <log_surgeon/Constants.hpp>

using std::string;

namespace log_surgeon {
FileReader::~FileReader() {
    close();
    free(m_get_delim_buf);
}

auto FileReader::read(char* buf, size_t const num_bytes_to_read, size_t& num_bytes_read) const
        -> ErrorCode {
    if (nullptr == m_file) {
        return ErrorCode::NotInit;
    }
    if (nullptr == buf) {
        return ErrorCode::BadParam;
    }
    num_bytes_read = fread(buf, sizeof(*buf), num_bytes_to_read, m_file);
    if (num_bytes_read < num_bytes_to_read) {
        if (0 != ferror(m_file)) {
            return ErrorCode::Errno;
        }
        if (0 != feof(m_file)) {
            if (0 == num_bytes_read) {
                return ErrorCode::EndOfFile;
            }
        }
    }
    return ErrorCode::Success;
}

auto FileReader::try_open(string const& path) -> ErrorCode {
    // Cleanup in case caller forgot to call close before calling this function
    if (ErrorCode const err = close(); ErrorCode::Success != err) {
        return err;
    }
    m_file = fopen(path.c_str(), "rb");
    if (nullptr == m_file) {
        if (ENOENT == errno) {
            return ErrorCode::FileNotFound;
        }
        return ErrorCode::Errno;
    }
    return ErrorCode::Success;
}

auto FileReader::close() -> ErrorCode {
    if (m_file != nullptr) {
        if (0 != fclose(m_file)) {
            return ErrorCode::Errno;
        }
        m_file = nullptr;
    }
    return ErrorCode::Success;
}

auto FileReader::try_read_to_delimiter(
        char const delim,
        bool const keep_delimiter,
        bool const append,
        string& str
) -> ErrorCode {
    assert(nullptr != m_file);
    if (false == append) {
        str.clear();
    }
    ssize_t num_bytes_read = getdelim(&m_get_delim_buf, &m_get_delim_buf_len, delim, m_file);
    if (num_bytes_read < 1) {
        if (0 != ferror(m_file)) {
            return ErrorCode::Errno;
        }
        if (0 != feof(m_file)) {
            return ErrorCode::EndOfFile;
        }
    }
    if (false == keep_delimiter && delim == m_get_delim_buf[num_bytes_read - 1]) {
        --num_bytes_read;
    }
    str.append(m_get_delim_buf, num_bytes_read);
    return ErrorCode::Success;
}
}  // namespace log_surgeon

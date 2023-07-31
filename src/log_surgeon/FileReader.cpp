#include "FileReader.hpp"

#include <unistd.h>

#include <cassert>
#include <cerrno>

#include <log_surgeon/Constants.hpp>

using std::string;

namespace log_surgeon {
FileReader::~FileReader() {
    close();
    free(m_get_delim_buf);
}

auto FileReader::read(char* buf, size_t num_bytes_to_read, size_t& num_bytes_read) -> ErrorCode {
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
    close();
    m_file = fopen(path.c_str(), "rb");
    if (nullptr == m_file) {
        if (ENOENT == errno) {
            return ErrorCode::FileNotFound;
        }
        return ErrorCode::Errno;
    }
    return ErrorCode::Success;
}

auto FileReader::close() -> void {
    if (m_file != nullptr) {
        // NOTE: We don't check errors for fclose since it seems the only reason
        // it could fail is if it was interrupted by a signal
        fclose(m_file);
        m_file = nullptr;
    }
}

auto FileReader::try_read_to_delimiter(char delim, bool keep_delimiter, bool append, string& str)
        -> ErrorCode {
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

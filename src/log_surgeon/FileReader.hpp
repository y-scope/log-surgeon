#ifndef LOG_SURGEON_FILE_READER_HPP
#define LOG_SURGEON_FILE_READER_HPP

#include <cstdio>
#include <string>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/Reader.hpp>

namespace log_surgeon {
class FileReader : public Reader {
public:
    ~FileReader();
    /**
     * Tries to read up to a given number of bytes from the file
     * @param buf
     * @param num_bytes_to_read The number of bytes to try and read
     * @param num_bytes_read The actual number of bytes read
     * @return ErrorCode::NotInit if the file is not open
     * @return ErrorCode::BadParam if buf is invalid
     * @return ErrorCode::Errno on error
     * @return ErrorCode::EndOfFile on EOF
     * @return ErrorCode::Success on success
     */
    auto read(char* buf, size_t num_bytes_to_read, size_t& num_bytes_read) -> ErrorCode;

    /**
     * Tries to read a string from the file until it reaches the specified
     * delimiter
     * @param delim The delimiter to stop at
     * @param keep_delimiter Whether to include the delimiter in the output
     * string or not
     * @param append Whether to append to the given string or replace its
     * contents
     * @param str The string read
     * @return ErrorCode::Success on success
     * @return ErrorCode::EndOfFile on EOF
     * @return ErrorCode::Errno otherwise
     */
    auto try_read_to_delimiter(char delim, bool keep_delimiter, bool append, std::string& str)
            -> ErrorCode;

    /**
     * Tries to open a file
     * @param path
     * @return ErrorCode::Success on success
     * @return ErrorCode::FileNotFound if the file was not found
     * @return ErrorCode::Errno otherwise
     */
    auto try_open(std::string const& path) -> ErrorCode;

    /**
     * Closes the file if it's open
     */
    auto close() -> void;

private:
    FILE* m_file{nullptr};
    size_t m_get_delim_buf_len{0};
    char* m_get_delim_buf{nullptr};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_FILE_READER_HPP

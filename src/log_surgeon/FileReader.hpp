#ifndef LOG_SURGEON_FILE_READER_HPP
#define LOG_SURGEON_FILE_READER_HPP

#include <cstdint>
#include <fstream>
#include <string>
#include <variant>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/Reader.hpp>

namespace log_surgeon {
/// TODO: rename to SchemaFileReader and move to file called SchemaFileReader.hpp
class FileReader : public Reader {
public:
    /**
     * Read the given number of bytes from the file
     * @param buf
     * @param num_bytes_to_read The number of bytes to try and read
     * @param num_bytes_read The actual number of bytes read
     * @return ErrorCode::NotInit if the file is not open
     * @return ErrorCode::BadParam if buf is invalid
     * @return ErrorCode::Errno on error
     * @return ErrorCode::EndOfFile on EOF
     * @return ErrorCode::Success on success
     */
    auto read(char* buf, uint32_t num_bytes_to_read, uint32_t& num_bytes_read) -> ErrorCode;

    /**
     * Read the desired desired line from the file
     * @param schema_path
     * @param line_num
     * @return The string at the desired line if successful
     * @return ErrorCode::FileNotFound if the file was not found
     * @return ErrorCode::EndOfFile on EOF
     * @return ErrorCode::Errno otherwise
     */
    auto open_and_read_to_line_number(std::string const& schema_path, uint32_t line_num)
            -> std::variant<std::string, ErrorCode>;

    /**
     * Opens the file
     * @param path
     * @return ErrorCode::Success on success
     * @return ErrorCode::FileNotFound if the file was not found
     * @return ErrorCode::Errno otherwise
     */
    auto open(std::string const& path) -> ErrorCode;

    /**
     * Closes the file
     * @return ErrorCode::Success on success
     * @return ErrorCode::Errno otherwise
     */
    auto close() -> ErrorCode;

private:
    std::ifstream m_file_stream;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_FILE_READER_HPP

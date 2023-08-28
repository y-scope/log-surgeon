#include "ParserInputBuffer.hpp"

#include <memory>
#include <string>

#include <log_surgeon/Constants.hpp>

using std::string;
using std::to_string;

namespace log_surgeon {
void ParserInputBuffer::reset() {
    m_log_fully_consumed = false;
    m_finished_reading_input = false;
    m_pos_last_read_char = 0;
    m_last_read_first_half = false;
    m_storage.reset();
    m_consumed_pos = m_storage.size() - 1;
}

auto ParserInputBuffer::read_is_safe() -> bool {
    if (m_finished_reading_input) {
        return false;
    }
    // Check if the last log message ends in the buffer half last read.
    // This means the other half of the buffer has already been fully used.
    if ((!m_last_read_first_half && m_consumed_pos > m_storage.size() / 2)
        || (m_last_read_first_half && m_consumed_pos < m_storage.size() / 2 && m_consumed_pos > 0))
    {
        return true;
    }
    return false;
}

auto ParserInputBuffer::read(Reader& reader) -> ErrorCode {
    size_t bytes_read{0};
    // read into the correct half of the buffer
    uint32_t read_offset{0};
    if (m_last_read_first_half) {
        read_offset = m_storage.size() / 2;
    }
    if (ErrorCode err = m_storage.read(reader, read_offset, m_storage.size() / 2, bytes_read);
        ErrorCode::Success != err)
    {
        if (ErrorCode::EndOfFile == err) {
            m_finished_reading_input = true;
        }
        return err;
    }
    m_last_read_first_half = !m_last_read_first_half;
    // TODO: This is not a portable check for certain forms of IO
    // A method from Reader should be used to check if the input source is
    // finished
    if (bytes_read < m_storage.size() / 2) {
        m_finished_reading_input = true;
    }
    m_pos_last_read_char += bytes_read;
    if (m_pos_last_read_char > m_storage.size()) {
        m_pos_last_read_char -= m_storage.size();
    }
    return ErrorCode::Success;
}

auto ParserInputBuffer::increase_capacity(uint32_t& old_storage_size, bool& flipped_static_buffer)
        -> void {
    old_storage_size = m_storage.size();
    uint32_t new_storage_size = old_storage_size * 2;
    flipped_static_buffer = false;
    char const* old_storage = m_storage.get_active_buffer();
    m_storage.double_size();
    if (m_last_read_first_half == false) {
        // Buffer in correct order
        m_storage.copy(old_storage, old_storage + old_storage_size, 0);
    } else {
        uint32_t half_old_storage_size = old_storage_size / 2;
        // Buffer out of order, so it needs to be flipped when copying
        m_storage.copy(old_storage + half_old_storage_size, old_storage + old_storage_size, 0);
        m_storage.copy(old_storage, old_storage + half_old_storage_size, half_old_storage_size);
        flipped_static_buffer = true;
    }
    m_last_read_first_half = true;
    m_pos_last_read_char = new_storage_size - old_storage_size;
    m_storage.set_pos(old_storage_size);
}

auto ParserInputBuffer::get_next_character(unsigned char& next_char) -> ErrorCode {
    if (m_finished_reading_input && m_storage.pos() == m_pos_last_read_char) {
        m_log_fully_consumed = true;
        next_char = utf8::cCharEOF;
        return ErrorCode::Success;
    }
    if ((m_last_read_first_half && m_storage.pos() == m_storage.size() / 2)
        || (!m_last_read_first_half && m_storage.pos() == 0))
    {
        return ErrorCode::BufferOutOfBounds;
    }
    char character = m_storage.get_curr_value();
    m_storage.increment_pos();
    if (m_storage.pos() == m_storage.size()) {
        m_storage.set_pos(0);
    }
    next_char = character;
    return ErrorCode::Success;
}

// This is a work around to allow the BufferParser to work without needing
// the user to wrap their input buffer. It tricks the LogParser and
// ParserInputBuffer into thinking it never reaches the wrap, while still
// respecting the actual size of the buffer the user passed in.
void ParserInputBuffer::set_storage(
        char* storage,
        uint32_t size,
        uint32_t pos,
        bool finished_reading_input
) {
    reset();
    m_storage.set_active_buffer(storage, size * 2, pos);
    m_finished_reading_input = finished_reading_input;
    m_pos_last_read_char = size;
    m_last_read_first_half = true;
}
}  // namespace log_surgeon

#ifndef LOG_SURGEON_PARSER_INPUT_BUFFER_HPP
#define LOG_SURGEON_PARSER_INPUT_BUFFER_HPP

// Project Headers
#include "Buffer.hpp"
#include "Constants.hpp"

namespace log_surgeon {
/**
 * A buffer containing a log segment as a sequence of characters. Half of
 * the buffer is read into at a time, keeping track of the current position,
 * last half read into, last position read into, and what position the
 * caller has already consumed (indicating which characters are no longer
 * needed by the caller). A half is only read into if it has been fully
 * consumed, such that no unused data is overwritten. For performance
 * (runtime latency) it defaults to a static buffer and when more characters
 * are needed to represent a log message it switches to a dynamic buffer.
 * Each time the buffer is completely read without matching a log message,
 * more data is read in from the log into a new dynamic buffer with double
 * the current capacity.
 */
class ParserInputBuffer {
public:
    /**
     * Reset the underlying storage and zeroing all tracking state.
     */
    auto reset() -> void;

    /**
     * Checks if reading into the buffer will only overwrite consumed data.
     * @return bool
     */
    auto read_is_safe() -> bool;

    /**
     * Reads if only consumed data will be overwritten.
     * @param reader to use for IO.
     * @return ErrorCode::Success on successful read or if a read is unsafe.
     * @return ErrorCode forwarded from read.
     */
    auto read_if_safe(Reader& reader) -> ErrorCode {
        if (read_is_safe()) {
            return read(reader);
        }
        return ErrorCode::Success;
    }

    /**
     * Creates a new dynamic buffer with double the capacity. The first
     * half of the new buffer contains the old content in the same order
     * as in the original log. As the buffers are read into half at a time,
     * this may require reordering the two halves of the old buffer if the
     * content stored in the second half precedes the content stored in the
     * first half in the original log.
     * @param old_storage_size
     * @param flipped_static_buffer
     */
    auto increase_capacity(uint32_t& old_storage_size, bool& flipped_static_buffer) -> void;

    /**
     * Attempt to get the next character from the input buffer.
     * @param next_char Populated with the next character in the input buffer.
     * @return ErrorCode::Success
     * @return ErrorCode::BufferOutOfBounds if the end of the input buffer has
     * been reached.
     */
    auto get_next_character(unsigned char& next_char) -> ErrorCode;

    /**
     * Set the current position of the underlying buffer.
     * @param pos The value to set.
     */
    auto set_pos(uint32_t pos) -> void { m_storage.set_pos(pos); }

    /**
     * Set the consumed position of the underlying buffer. This position
     * represents the point in the input buffer at which the data has been
     * fully parsed and is no longer necessary.
     * @param consumed_pos The value to set.
     */
    auto set_consumed_pos(uint32_t consumed_pos) -> void { m_consumed_pos = consumed_pos; }

    /**
     * Set whether the log input source has been fully consumed. True when
     * there is both no more input (m_finished_reading_input is true) and the
     * end of the buffer has been reached.
     * @param log_fully_consumed The value to set.
     */
    auto set_log_fully_consumed(bool log_fully_consumed) -> void {
        m_log_fully_consumed = log_fully_consumed;
    }

    /**
     * Returns whether the log input source has been fully consumed. True when
     * there is both no more input (m_finished_reading_input is true) and the
     * end of the buffer has been reached.
     */
    [[nodiscard]] auto log_fully_consumed() const -> bool { return m_log_fully_consumed; }

    /**
     * Manually setup the underlying storage buffer. The ParserInputBuffer will
     * no longer use the currently set underlying storage and instead use what
     * is passed in.
     * @param storage A pointer to the new buffer to use as the input buffer.
     * @param size The size of the buffer pointed to by storage.
     * @param pos The current position into the buffer (storage) to set.
     * @param finished_reading_input True if there is no more input left to
     * process, so the parser will finish consuming the entire buffer without
     * requesting more input data.
     */
    auto set_storage(char* storage, uint32_t size, uint32_t pos, bool finished_reading_input)
            -> void;

    /**
     * Return a reference to the underlying storage buffer.
     */
    [[nodiscard]] auto storage() const -> Buffer<char> const& { return m_storage; }

private:
    /**
     * Reads into the half of the buffer currently available.
     * @param reader to use for IO.
     */
    auto read(Reader& reader) -> ErrorCode;

private:
    // the position of the last character read into the buffer
    uint32_t m_pos_last_read_char{0};
    bool m_last_read_first_half{false};
    // the log has been completely read into the buffer
    bool m_finished_reading_input{false};
    // the buffer has finished iterating over the entire log
    bool m_log_fully_consumed{false};
    // contains the static and dynamic character buffers
    Buffer<char> m_storage{};
    // the position last used by the caller (no longer needed in storage)
    uint32_t m_consumed_pos{m_storage.size() - 1};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_PARSER_INPUT_BUFFER_HPP

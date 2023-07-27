#include "LogParserOutputBuffer.hpp"

#include <string>

using std::string;

namespace log_surgeon {
auto LogParserOutputBuffer::advance_to_next_token() -> void {
    m_storage.increment_pos();
    if (m_storage.pos() == m_storage.size()) {
        Token const* old_storage = m_storage.get_active_buffer();
        uint32_t old_size = m_storage.size();
        m_storage.double_size();
        m_storage.copy(old_storage, old_storage + old_size, 0);
    }
}
}  // namespace log_surgeon

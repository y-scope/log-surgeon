#ifndef LOG_SURGEON_BUFFER_HPP
#define LOG_SURGEON_BUFFER_HPP

#include <cstdint>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/Reader.hpp>

namespace log_surgeon {
/**
 * A base class for the efficient implementation of a single growing buffer.
 * Under the hood it keeps track of one static buffer and multiple dynamic
 * buffers. The buffer object uses the underlying static buffer whenever
 * possible, as the static buffer is on the stack and results in faster reads
 * and writes. In outlier cases, where the static buffer is not large enough to
 * fit all the needed data, the buffer object switches to using the underlying
 * dynamic buffers. A new dynamic buffer is used each time the size must be
 * grown to preserve any pointers to the buffer. All pointers to the buffer are
 * valid until reset() is called and the buffer returns to using the underlying
 * static buffer. The base class does not grow the buffer itself, the child
 * class is responsible for doing this.
 */
template <typename Item>
class Buffer {
public:
    auto set_curr_value(Item const& value) -> void { m_active_storage[m_pos] = value; }

    [[nodiscard]] auto get_curr_value() const -> Item const& { return m_active_storage[m_pos]; }

    auto set_value(uint32_t pos, Item const& value) -> void { m_active_storage[pos] = value; }

    [[nodiscard]] auto get_value(uint32_t pos) const -> Item const& {
        return m_active_storage[pos];
    }

    [[nodiscard]] auto get_mutable_value(uint32_t pos) const -> Item& {
        return m_active_storage[pos];
    }

    auto increment_pos() -> void { m_pos++; }

    auto set_pos(uint32_t curr_pos) -> void { m_pos = curr_pos; }

    [[nodiscard]] auto pos() const -> uint32_t { return m_pos; }

    auto double_size() -> void {
        std::vector<Item>& new_buffer = m_dynamic_storages.emplace_back(2 * m_active_size);
        m_active_storage = new_buffer.data();
        m_active_size *= 2;
    }

    [[nodiscard]] auto static_size() const -> uint32_t { return cStaticByteBuffSize; }

    [[nodiscard]] auto size() const -> uint32_t { return m_active_size; }

    auto reset() -> void {
        m_pos = 0;
        m_dynamic_storages.clear();
        m_active_storage = m_static_storage;
        m_active_size = cStaticByteBuffSize;
    }

    auto set_active_buffer(Item* storage, uint32_t size, uint32_t pos) -> void {
        m_active_storage = storage;
        m_active_size = size;
        m_pos = pos;
    }

    [[nodiscard]] auto get_active_buffer() const -> Item const* { return m_active_storage; }

    // Currently needed for compression
    [[nodiscard]] auto get_mutable_active_buffer() -> Item* { return m_active_storage; }

    void
    copy(Item const* storage_to_copy_first, Item const* storage_to_copy_last, uint32_t offset) {
        std::copy(storage_to_copy_first, storage_to_copy_last, m_active_storage + offset);
    }

    template <class T>
    auto read(T& reader, uint32_t read_offset, uint32_t bytes_to_read, size_t& bytes_read)
            -> ErrorCode {
        static_assert(std::is_same_v<T, Reader>, "T should be a Reader");
        return reader.read(m_active_storage + read_offset, bytes_to_read, bytes_read);
    }

private:
    uint32_t m_pos{0};
    uint32_t m_active_size{cStaticByteBuffSize};
    std::vector<std::vector<Item>> m_dynamic_storages;
    Item m_static_storage[cStaticByteBuffSize];
    Item* m_active_storage{m_static_storage};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_BUFFER_HPP

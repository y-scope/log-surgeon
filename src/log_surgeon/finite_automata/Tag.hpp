#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAG
#define LOG_SURGEON_FINITE_AUTOMATA_TAG

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace log_surgeon::finite_automata {
/**
 * This class represents a tag that is associated with matches of a capture group. If
 * `m_start_positions` is empty, it indicates that the capture group was unmatched.
 *
 * Since capture group regex can be contained within repetition regex,
 * (e.g., "((user_id=(?<uid>\d+),)+"), `m_start_positions` and `m_end_positions` are vectors that
 * track the locations of each occurrence of the capture group.
 */
class Tag {
public:
    explicit Tag(std::string name) : m_name{std::move(name)} {}

    auto set_start_positions(std::vector<uint32_t> start_positions) -> void {
        m_start_positions = std::move(start_positions);
    }

    auto set_end_positions(std::vector<uint32_t> end_positions) -> void {
        m_end_positions = std::move(end_positions);
    }

    [[nodiscard]] auto get_name() const -> std::string const& { return m_name; }

private:
    std::string const m_name;
    std::vector<uint32_t> m_start_positions;
    std::vector<uint32_t> m_end_positions;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAG

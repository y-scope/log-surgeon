#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAG
#define LOG_SURGEON_FINITE_AUTOMATA_TAG

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace log_surgeon::finite_automata {
/**
 * This class represents a tag that is associated with matches of a capture group. If `m_starts` is
 * empty, it indicates that the capture group was unmatched.
 *
 * Since capture group regex can be contained within repetition regex,
 * (e.g., "((user_id=(?<uid>\d+),)+"), `m_starts` and `m_ends` are vectors that track the locations
 * of each occurrence of the capture group.
 */
class Tag {
public:
    explicit Tag(std::string name) : m_name{std::move(name)} {}

    [[nodiscard]] auto get_name() const -> std::string const& { return m_name; }

private:
    std::string const m_name;
    std::vector<uint32_t> m_starts;
    std::vector<uint32_t> m_ends;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAG

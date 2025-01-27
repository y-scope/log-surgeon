#ifndef LOG_SURGEON_FINITE_AUTOMATA_SPONTANEOUS_TRANSITION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_SPONTANEOUS_TRANSITION_HPP

#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <tuple>
#include <unordered_map>
#include <utility>
#include <vector>

#include <fmt/format.h>

#include <log_surgeon/finite_automata/TagOperation.hpp>

namespace log_surgeon::finite_automata {
/**
 * Represents an NFA transition indicating a tag and an operation to perform on the tag.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class SpontaneousTransition {
public:
    explicit SpontaneousTransition(TypedNfaState const* dest_state) : m_dest_state{dest_state} {}

    SpontaneousTransition(std::vector<TagOperation> tag_ops, TypedNfaState const* dest_state)
            : m_tag_ops{std::move(tag_ops)},
              m_dest_state{dest_state} {}

    auto operator<(SpontaneousTransition const& rhs) const -> bool {
        return std::tie(m_tag_ops, m_dest_state) < std::tie(rhs.m_tag_ops, rhs.m_dest_state);
    }

    [[nodiscard]] auto get_tag_ops() const -> std::vector<TagOperation> { return m_tag_ops; }

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the spontaneous transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    std::vector<TagOperation> m_tag_ops;
    TypedNfaState const* m_dest_state;
};

template <typename TypedNfaState>
auto SpontaneousTransition<TypedNfaState>::serialize(
        std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto const state_id_it = state_ids.find(m_dest_state);
    if (state_id_it == state_ids.end()) {
        return std::nullopt;
    }
    auto transformed_operations
            = m_tag_ops | std::ranges::views::transform([](TagOperation const& tag_op) {
                  return tag_op.serialize();
              });

    return fmt::format("{}[{}]", state_id_it->second, fmt::join(transformed_operations, ","));
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_SPONTANEOUS_TRANSITION_HPP

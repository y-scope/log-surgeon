#ifndef LOG_SURGEON_FINITE_AUTOMATA_NFASPONTANEOUSTRANSITION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_NFASPONTANEOUSTRANSITION_HPP

#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/TagOperation.hpp>

#include <fmt/format.h>

namespace log_surgeon::finite_automata {
/**
 * Represents an NFA transition with a collection of tag operations to be performed during the
 * transition.
 *
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class NfaSpontaneousTransition {
public:
    NfaSpontaneousTransition(std::vector<TagOperation> tag_ops, TypedNfaState const* dest_state)
            : m_tag_ops{std::move(tag_ops)},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_tag_ops() const -> std::vector<TagOperation> const& { return m_tag_ops; }

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the spontaneous transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(
            std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    std::vector<TagOperation> m_tag_ops;
    TypedNfaState const* m_dest_state;
};

template <typename TypedNfaState>
auto NfaSpontaneousTransition<TypedNfaState>::serialize(
        std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    if (false == state_ids.contains(m_dest_state)) {
        return std::nullopt;
    }
    auto transformed_operations
            = m_tag_ops | std::ranges::views::transform(&TagOperation::serialize);

    return fmt::format(
            "{}[{}]",
            state_ids.at(m_dest_state),
            fmt::join(transformed_operations, ",")
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_NFASPONTANEOUSTRANSITION_HPP

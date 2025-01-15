#ifndef LOG_SURGEON_FINITE_AUTOMATA_SPONTANEOUS_TRANSITION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_SPONTANEOUS_TRANSITION_HPP

#include <algorithm>
#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>

#include <fmt/format.h>

namespace log_surgeon::finite_automata {
using tag_id_t = std::uint32_t;

enum class TransitionOperation {
    None,
    SetTags,
    NegateTags
};

/**
 * Represents an NFA transition indicating a tag and an operation to perform on the tag.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class SpontaneousTransition {
public:
    SpontaneousTransition(TypedNfaState const* dest_state)
            : m_transition_op{TransitionOperation::None},
              m_dest_state{dest_state} {}

    SpontaneousTransition(
            TransitionOperation const transition_op,
            std::vector<tag_id_t> tag_ids,
            TypedNfaState const* dest_state
    )
            : m_transition_op{transition_op},
              m_tag_ids{std::move(tag_ids)},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_transition_operation() const -> TransitionOperation {
        return m_transition_op;
    }

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string> {
        auto const state_id_it = state_ids.find(m_dest_state);
        if (state_id_it == state_ids.end()) {
            return std::nullopt;
        }
        std::string transition_op_string;
        if (TransitionOperation::SetTags == m_transition_op) {
            transition_op_string = "set:";
        } else if (TransitionOperation::NegateTags == m_transition_op) {
            transition_op_string = "negate:";
        }
        return fmt::format(
                "{}[{}{}]",
                state_id_it->second,
                transition_op_string,
                fmt::join(m_tag_ids, ",")
        );
    }

private:
    TransitionOperation const m_transition_op;
    std::vector<tag_id_t> const m_tag_ids;
    TypedNfaState const* m_dest_state;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_SPONTANEOUS_TRANSITION_HPP

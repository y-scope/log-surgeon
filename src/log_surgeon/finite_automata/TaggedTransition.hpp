#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION
#define LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

#include <algorithm>
#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <unordered_map>

#include <fmt/format.h>

namespace log_surgeon::finite_automata {
using tag_id_t = std::uint32_t;

/**
 * Represents an NFA transition indicating that a tag has been matched.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class PositiveTaggedTransition {
public:
    PositiveTaggedTransition(tag_id_t const tag_id, TypedNfaState const* dest_state)
            : m_tag_id{tag_id},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the positive tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string> {
        auto const state_id_it = state_ids.find(m_dest_state);
        if (state_id_it == state_ids.end()) {
            return std::nullopt;
        }
        return fmt::format("{}[{}]", state_id_it->second, m_tag_id);
    }

private:
    tag_id_t m_tag_id;
    TypedNfaState const* m_dest_state;
};

/**
 * Represents an NFA transition indicating that a tag has been unmatched.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class NegativeTaggedTransition {
public:
    NegativeTaggedTransition(std::vector<tag_id_t> const tag_ids, TypedNfaState const* dest_state)
            : m_tag_ids{tag_ids},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the negative tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string> {
        auto const state_id_it = state_ids.find(m_dest_state);
        if (state_id_it == state_ids.end()) {
            return std::nullopt;
        }
        return fmt::format("{}[{}]", state_id_it->second, fmt::join(m_tag_ids, ","));
    }

private:
    std::vector<tag_id_t> m_tag_ids;
    TypedNfaState const* m_dest_state;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

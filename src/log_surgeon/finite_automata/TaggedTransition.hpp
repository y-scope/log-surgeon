#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION
#define LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <unordered_map>

#include <fmt/format.h>

#include <log_surgeon/finite_automata/Tag.hpp>

namespace log_surgeon::finite_automata {

/**
 * Represents an NFA transition indicating that a capture group has been matched.
 * NOTE: `m_tag` is always expected to be non-null.
 * @tparam NFAStateType Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename NFAStateType>
class PositiveTaggedTransition {
public:
    /**
     * @param tag
     * @param dest_state
     * @throw std::invalid_argument if `tag` is `nullptr`.
     */
    PositiveTaggedTransition(Tag* tag, NFAStateType const* dest_state)
            : m_tag{nullptr == tag ? throw std::invalid_argument("Tag cannot be null") : tag},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_dest_state() const -> NFAStateType const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the positive tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<NFAStateType const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string> {
        auto const state_id_it = state_ids.find(m_dest_state);
        if (state_id_it == state_ids.end()) {
            return std::nullopt;
        }
        return fmt::format("{}[{}]", state_id_it->second, m_tag->get_name());
    }

private:
    Tag* m_tag;
    NFAStateType const* m_dest_state;
};

/**
 * Represents an NFA transition indicating that a capture group has been unmatched.
 * NOTE: All tags in `m_tags` are always expected to be non-null.
 * @tparam NFAStateType Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename NFAStateType>
class NegativeTaggedTransition {
public:
    /**
     * @param tags
     * @param dest_state
     * @throw std::invalid_argument if any elements in `tags` is `nullptr`.
     */
    NegativeTaggedTransition(std::vector<Tag*> tags, NFAStateType* dest_state)
            : m_tags{[&tags] {
                  if (std::ranges::any_of(tags, [](Tag const* tag) { return nullptr == tag; })) {
                      throw std::invalid_argument("Tags cannot contain null elements");
                  }
                  return std::move(tags);
              }()},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_dest_state() const -> NFAStateType const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the negative tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<NFAStateType const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string> {
        auto const state_id_it = state_ids.find(m_dest_state);
        if (state_id_it == state_ids.end()) {
            return std::nullopt;
        }

        auto const tag_names = m_tags | std::ranges::views::transform(&Tag::get_name);

        return fmt::format("{}[{}]", state_id_it->second, fmt::join(tag_names, ","));
    }

private:
    std::vector<Tag*> m_tags;
    NFAStateType* m_dest_state;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

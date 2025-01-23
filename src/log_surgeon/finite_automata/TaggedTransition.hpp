#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION
#define LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

#include <algorithm>
#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <unordered_map>

#include <log_surgeon/finite_automata/Tag.hpp>

namespace log_surgeon::finite_automata {
/**
 * Represents an NFA transition indicating that a capture group has been matched.
 * NOTE: `m_tag` is always expected to be non-null.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class PositiveTaggedTransition {
public:
    /**
     * @param tag
     * @param dest_state
     * @throw std::invalid_argument if `tag` is `nullptr`.
     */
    PositiveTaggedTransition(Tag const* tag, TypedNfaState const* dest_state)
            : m_tag{nullptr == tag ? throw std::invalid_argument("Tag cannot be null") : tag},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the positive tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    Tag const* m_tag;
    TypedNfaState const* m_dest_state;
};

/**
 * Represents an NFA transition indicating that a capture group has been unmatched.
 * NOTE: All tags in `m_tags` are always expected to be non-null.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class NegativeTaggedTransition {
public:
    /**
     * @param tags
     * @param dest_state
     * @throw std::invalid_argument if any elements in `tags` is `nullptr`.
     */
    NegativeTaggedTransition(std::vector<Tag const*> tags, TypedNfaState const* dest_state)
            : m_tags{[&tags] {
                  if (std::ranges::any_of(tags, [](Tag const* tag) { return nullptr == tag; })) {
                      throw std::invalid_argument("Tags cannot contain null elements");
                  }
                  return std::move(tags);
              }()},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_dest_state() const -> TypedNfaState const* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the negative tagged transition on success.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    std::vector<Tag const*> m_tags;
    TypedNfaState const* m_dest_state;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

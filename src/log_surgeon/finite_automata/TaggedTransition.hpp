#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION
#define LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

#include <algorithm>
#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <unordered_map>

#include <fmt/format.h>

#include <log_surgeon/finite_automata/Capture.hpp>

namespace log_surgeon::finite_automata {
/**
 * Represents an NFA transition indicating that a capture group has been matched.
 * NOTE: `m_capture` is always expected to be non-null.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class PositiveTaggedTransition {
public:
    /**
     * @param capture
     * @param dest_state
     * @throw std::invalid_argument if `capture` is `nullptr`.
     */
    PositiveTaggedTransition(Capture const* capture, TypedNfaState const* dest_state)
            : m_capture{nullptr == capture ? throw std::invalid_argument("Capture cannot be null") : capture},
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
        return fmt::format("{}[{}]", state_id_it->second, m_capture->get_name());
    }

private:
    Capture const* m_capture;
    TypedNfaState const* m_dest_state;
};

/**
 * Represents an NFA transition indicating that a capture group has been unmatched.
 * NOTE: All captures in `m_captures` are always expected to be non-null.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class NegativeTaggedTransition {
public:
    /**
     * @param captures
     * @param dest_state
     * @throw std::invalid_argument if any elements in `captures` is `nullptr`.
     */
    NegativeTaggedTransition(std::vector<Capture const*> captures, TypedNfaState const* dest_state)
            : m_captures{[&captures] {
                  if (std::ranges::any_of(captures, [](Capture const* capture) {
                          return nullptr == capture;
                      }))
                  {
                      throw std::invalid_argument("Captures cannot contain null elements");
                  }
                  return std::move(captures);
              }()},
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

        auto const capture_names = m_captures | std::ranges::views::transform(&Capture::get_name);

        return fmt::format("{}[{}]", state_id_it->second, fmt::join(capture_names, ","));
    }

private:
    std::vector<Capture const*> m_captures;
    TypedNfaState const* m_dest_state;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAGGED_TRANSITION

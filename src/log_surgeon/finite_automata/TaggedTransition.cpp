#include "TaggedTransition.hpp"

#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
auto PositiveTaggedTransition<TypedNfaState>::serialize(
        std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto const state_id_it = state_ids.find(m_dest_state);
    if (state_id_it == state_ids.end()) {
        return std::nullopt;
    }
    return fmt::format("{}[{}]", state_id_it->second, m_tag->get_name());
}

template <typename TypedNfaState>
auto NegativeTaggedTransition<TypedNfaState>::serialize(
    std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto const state_id_it = state_ids.find(m_dest_state);
    if (state_id_it == state_ids.end()) {
        return std::nullopt;
    }

    auto const tag_names = m_tags | std::ranges::views::transform(&Tag::get_name);

    return fmt::format("{}[{}]", state_id_it->second, fmt::join(tag_names, ","));
}

// Explicit template instantiations
template auto PositiveTaggedTransition<ByteNfaState>::serialize(
        std::unordered_map<ByteNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;
template auto PositiveTaggedTransition<Utf8NfaState>::serialize(
        std::unordered_map<Utf8NfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;
template auto NegativeTaggedTransition<ByteNfaState>::serialize(
        std::unordered_map<ByteNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;
template auto NegativeTaggedTransition<Utf8NfaState>::serialize(
        std::unordered_map<Utf8NfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;

} // namespace log_surgeon::finite_automata
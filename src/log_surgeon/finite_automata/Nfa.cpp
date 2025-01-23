#include "Nfa.hpp"

#include <fmt/core.h>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
auto Nfa<TypedNfaState>::serialize() const -> std::string {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<TypedNfaState const*, uint32_t> state_ids;
    for (auto const* state : traversal_order) {
        state_ids.emplace(state, state_ids.size());
    }

    std::vector<std::string> serialized_states;
    for (auto const* state : traversal_order) {
        // `state_ids` is well-formed as its generated from `get_bfs_traversal_order` so we can
        // safely assume `state->serialize(state_ids)` will return a valid value.
        // NOLINTNEXTLINE(bugprone-unchecked-optional-access)
        serialized_states.emplace_back(state->serialize(state_ids).value());
    }
    return fmt::format("{}\n", fmt::join(serialized_states, "\n"));
}

// Explicit template instatiations
template auto Nfa<ByteNfaState>::serialize() const -> std::string;
template auto Nfa<Utf8NfaState>::serialize() const -> std::string;

}  // namespace log_surgeon::finite_automata
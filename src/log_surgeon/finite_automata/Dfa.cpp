#include "Dfa.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>

#include <fmt/core.h>
#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::serialize() const -> std::optional<std::string> {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<TypedDfaState const*, uint32_t> state_ids;
    state_ids.reserve(traversal_order.size());
    for (auto const* state : traversal_order) {
        state_ids.emplace(state, state_ids.size());
    }

    std::vector<std::string> serialized_states;
    for (auto const* state : traversal_order) {
        auto const optional_serialized_state{state->serialize(state_ids)};
        if (false == optional_serialized_state.has_value()) {
            return std::nullopt;
        }
        serialized_states.emplace_back(optional_serialized_state.value());
    }
    return fmt::format("{}\n", fmt::join(serialized_states, "\n"));
}

template auto Dfa<ByteDfaState, ByteNfaState>::serialize() const -> std::optional<std::string>;
template auto Dfa<Utf8DfaState, Utf8NfaState>::serialize() const -> std::optional<std::string>;
}  // namespace log_surgeon::finite_automata

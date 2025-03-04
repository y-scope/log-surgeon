#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <stack>
#include <vector>

#include <fmt/core.h>
#include <fmt/format.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/DfaStatePair.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>

namespace log_surgeon::finite_automata {
template <typename TypedDfaState, typename TypedNfaState>
class Dfa {
public:
    explicit Dfa(Nfa<TypedNfaState> const& nfa);

    /**
     * @return A string representation of the DFA.
     * @return Forwards `DfaState::serialize`'s return value (std::nullopt) on failure.
     */
    [[nodiscard]] auto serialize() const -> std::optional<std::string>;

    /**
     * Creates a new DFA state based on a set of NFA states and adds it to `m_states`.
     * @param nfa_state_set The set of NFA states represented by this DFA state.
     * @return A pointer to the new DFA state.
     */
    auto new_state(std::set<TypedNfaState const*> const& nfa_state_set) -> TypedDfaState*;

    auto get_root() const -> TypedDfaState const* { return m_states.at(0).get(); }

    /**
     * Compares this dfa with `dfa_in` to determine the set of schema types in this dfa that are
     * reachable by any type in `dfa_in`. A type is considered reachable if there is at least one
     * string for which: (1) this dfa returns a set of types containing the type, and (2) `dfa_in`
     * returns any non-empty set of types.
     *
     * @param dfa_in The dfa with which to take the intersect.
     * @return The set of schema types reachable by `dfa_in`.
     */
    [[nodiscard]] auto get_intersect(Dfa const* dfa_in) const -> std::set<uint32_t>;

private:
    /**
     * @return A vector representing the traversal order of the DFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<TypedDfaState const*>;

    std::vector<std::unique_ptr<TypedDfaState>> m_states;
};

template <typename TypedDfaState, typename TypedNfaState>
Dfa<TypedDfaState, TypedNfaState>::Dfa(Nfa<TypedNfaState> const& nfa) {
    using StateSet = std::set<TypedNfaState const*>;

    std::map<StateSet, TypedDfaState*> dfa_states;
    std::stack<StateSet> unmarked_sets;
    auto create_dfa_state
            = [this, &dfa_states, &unmarked_sets](StateSet const& set) -> TypedDfaState* {
        auto* state = new_state(set);
        dfa_states[set] = state;
        unmarked_sets.push(set);
        return state;
    };

    auto start_set = nfa.get_root()->epsilon_closure();
    create_dfa_state(start_set);
    while (false == unmarked_sets.empty()) {
        auto set = unmarked_sets.top();
        unmarked_sets.pop();
        auto* dfa_state = dfa_states.at(set);
        std::map<uint8_t, StateSet> ascii_transitions_map;
        // map<Interval, StateSet> transitions_map;
        for (auto const* s0 : set) {
            for (uint16_t i{0}; i < cSizeOfByte; ++i) {
                for (auto* const s1 : s0->get_byte_transitions(i)) {
                    StateSet closure = s1->epsilon_closure();
                    ascii_transitions_map[i].insert(closure.begin(), closure.end());
                }
            }
            // TODO: add this for the utf8 case
        }
        auto next_dfa_state
                = [&dfa_states, &create_dfa_state](StateSet const& set) -> TypedDfaState* {
            TypedDfaState* state;
            auto it = dfa_states.find(set);
            if (it == dfa_states.end()) {
                state = create_dfa_state(set);
            } else {
                state = it->second;
            }
            return state;
        };
        for (auto const& [byte, nfa_state_set] : ascii_transitions_map) {
            auto* dest_state{next_dfa_state(nfa_state_set)};
            dfa_state->add_byte_transition(byte, {{}, dest_state});
        }
        // TODO: add this for the utf8 case
    }
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::new_state(
        std::set<TypedNfaState const*> const& nfa_state_set
) -> TypedDfaState* {
    m_states.emplace_back(std::make_unique<TypedDfaState>());
    auto* dfa_state = m_states.back().get();
    for (auto const* nfa_state : nfa_state_set) {
        if (nfa_state->is_accepting()) {
            dfa_state->add_matching_variable_id(nfa_state->get_matching_variable_id());
        }
    }
    return dfa_state;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::get_intersect(Dfa const* dfa_in
) const -> std::set<uint32_t> {
    std::set<uint32_t> schema_types;
    std::set<DfaStatePair<TypedDfaState>> unvisited_pairs;
    std::set<DfaStatePair<TypedDfaState>> visited_pairs;
    unvisited_pairs.emplace(get_root(), dfa_in->get_root());
    // TODO: Handle UTF-8 (multi-byte transitions) as well
    while (false == unvisited_pairs.empty()) {
        auto current_pair_it = unvisited_pairs.begin();
        if (current_pair_it->is_accepting()) {
            auto const& matching_variable_ids = current_pair_it->get_matching_variable_ids();
            schema_types.insert(matching_variable_ids.cbegin(), matching_variable_ids.cend());
        }
        visited_pairs.insert(*current_pair_it);
        current_pair_it->get_reachable_pairs(visited_pairs, unvisited_pairs);
        unvisited_pairs.erase(current_pair_it);
    }
    return schema_types;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::get_bfs_traversal_order(
) const -> std::vector<TypedDfaState const*> {
    std::queue<TypedDfaState const*> state_queue;
    std::unordered_set<TypedDfaState const*> visited_states;
    std::vector<TypedDfaState const*> visited_order;
    visited_states.reserve(m_states.size());
    visited_order.reserve(m_states.size());

    auto try_add_to_queue_and_visited
            = [&state_queue, &visited_states](TypedDfaState const* dest_state) {
                  if (visited_states.insert(dest_state).second) {
                      state_queue.push(dest_state);
                  }
              };

    try_add_to_queue_and_visited(get_root());
    while (false == state_queue.empty()) {
        auto const* current_state = state_queue.front();
        visited_order.push_back(current_state);
        state_queue.pop();
        // TODO: Handle the utf8 case
        for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
            auto const& transition{current_state->get_transition(idx)};
            if (transition.has_value()) {
                auto const* dest_state{transition->get_dest_state()};
                try_add_to_queue_and_visited(dest_state);
            }
        }
    }
    return visited_order;
}

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
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

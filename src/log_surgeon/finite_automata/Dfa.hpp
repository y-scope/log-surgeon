#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

#include <cstdint>
#include <map>
#include <memory>
#include <set>
#include <stack>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/DfaStatePair.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>

namespace log_surgeon::finite_automata {
template <typename TypedDfaState, typename TypedNfaState>
class Dfa {
public:
    explicit Dfa(Nfa<TypedNfaState> nfa);

    auto get_root() const -> TypedDfaState const* { return m_states.at(0).get(); }

    /**
     * Compares this dfa with `dfa_in` to determine the set of schema types in this dfa that are
     * reachable by any type in `dfa_in`. A type is considered reachable if there is at least one
     * string for which: (1) this dfa returns a set of types containing the type, and (2) `dfa_in`
     * returns any non-empty set of types.
     * @param dfa_in The dfa with which to take the intersect.
     * @return The set of schema types reachable by `dfa_in`.
     */
    [[nodiscard]] auto get_intersect(Dfa const* dfa_in) const -> std::set<uint32_t>;

private:
    typedef std::set<TypedNfaState const*> NfaStateSet;

    /**
     * Generate the DFA states from the given NFA using the superset determinization algorithm.
     * @oaram nfa The NFA used to generate the DFA.
     */
    auto generate() -> void;

    /**
     * Creates a new DFA state based on a set of NFA states and adds it to `m_states`.
     * @param nfa_state_set The set of NFA states represented by this DFA state.
     * @return A pointer to the new DFA state.
     */
    auto new_state(NfaStateSet const& nfa_state_set) -> TypedDfaState*;

    std::vector<std::unique_ptr<TypedDfaState>> m_states;
    RegisterHandler m_register_handler;
    Nfa<TypedNfaState> m_nfa;
};

template <typename TypedDfaState, typename TypedNfaState>
Dfa<TypedDfaState, TypedNfaState>::Dfa(Nfa<TypedNfaState> nfa) : m_nfa(std::move(nfa)) {
    generate();
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::generate() -> void {
    std::map<NfaStateSet, TypedDfaState*> dfa_states;
    std::stack<NfaStateSet> unmarked_sets;
    auto create_dfa_state
            = [this, &dfa_states, &unmarked_sets](NfaStateSet const& set) -> TypedDfaState* {
        auto* state = new_state(set);
        dfa_states[set] = state;
        unmarked_sets.push(set);
        return state;
    };

    for (uint32_t i = 0; i < 4 * m_nfa.get_num_tags(); i++) {
        constexpr PrefixTree::position_t cDefaultPos{0};
        m_register_handler.add_register(PrefixTree::cRootId, cDefaultPos);
    }

    auto start_set = m_nfa.get_root()->epsilon_closure();

    create_dfa_state(start_set);
    while (false == unmarked_sets.empty()) {
        auto set = unmarked_sets.top();
        unmarked_sets.pop();
        auto* dfa_state = dfa_states.at(set);
        std::map<uint32_t, NfaStateSet> ascii_transitions_map;
        // map<Interval, NfaStateSet> transitions_map;
        for (auto const* s0 : set) {
            for (uint32_t i = 0; i < cSizeOfByte; i++) {
                for (auto* const s1 : s0->get_byte_transitions(i)) {
                    NfaStateSet closure = s1->epsilon_closure();
                    ascii_transitions_map[i].insert(closure.begin(), closure.end());
                }
            }
            // TODO: add this for the utf8 case
            /*
            for (auto const& data : s0->get_tree_transitions().all()) {
                for (auto* const s1 : data.m_value) {
                    NfaStateSet closure = s1->epsilon_closure();
                    transitions_map[data.m_interval].insert(closure.begin(), closure.end());
                }
            }
            */
        }
        auto next_dfa_state
                = [&dfa_states, &create_dfa_state](NfaStateSet const& set) -> TypedDfaState* {
            TypedDfaState* state{nullptr};
            auto it = dfa_states.find(set);
            if (it == dfa_states.end()) {
                state = create_dfa_state(set);
            } else {
                state = it->second;
            }
            return state;
        };
        for (auto const& kv : ascii_transitions_map) {
            auto* dest_state = next_dfa_state(kv.second);
            dfa_state->add_byte_transition(kv.first, dest_state);
        }
        // TODO: add this for the utf8 case
        /*
        for (auto& kv : transitions_map) {
            TypedDfaState* dest_state = next_dfa_state(kv.second);
            dfa_state->add_tree_transition(kv.first, dest_state);
        }
        */
    }
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::new_state(NfaStateSet const& nfa_state_set
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
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

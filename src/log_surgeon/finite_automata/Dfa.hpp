#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

#include <cstdint>
#include <map>
#include <memory>
#include <numeric>
#include <set>
#include <stack>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/DfaStatePair.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>

#include "DeterminizationConfiguration.hpp"

namespace log_surgeon::finite_automata {
template <typename TypedDfaState, typename TypedNfaState>
class Dfa {
public:
    explicit Dfa(Nfa<TypedNfaState> const& nfa);

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
    typedef std::set<DetermizationConfiguration<TypedNfaState>> ConfigurationSet;

    /**
     * Generate the DFA states from the given NFA using the superset determinization algorithm.
     * @oaram nfa The NFA used to generate the DFA.
     */
    auto generate(Nfa<TypedNfaState> const& nfa) -> void;

    /**
     * Creates a new DFA state based on a set of NFA states and adds it to `m_states`.
     * @param nfa_state_set The set of NFA states represented by this DFA state.
     * @return A pointer to the new DFA state.
     */
    auto new_state(ConfigurationSet const& nfa_state_set) -> TypedDfaState*;

    std::vector<std::unique_ptr<TypedDfaState>> m_states;
    RegisterHandler m_register_handler;
};

template <typename TypedDfaState, typename TypedNfaState>
Dfa<TypedDfaState, TypedNfaState>::Dfa(Nfa<TypedNfaState> const& nfa) {
    generate(nfa);
}

// TODO: handle utf8 case in DFA generation.
template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::generate(Nfa<TypedNfaState> const& nfa) -> void {
    std::map<ConfigurationSet, TypedDfaState*> dfa_states;
    std::stack<ConfigurationSet> unexplored_sets;
    auto create_dfa_state
            = [this, &dfa_states, &unexplored_sets](ConfigurationSet const& set) -> TypedDfaState* {
        auto* state = new_state(set);
        dfa_states[set] = state;
        unexplored_sets.push(set);
        return state;
    };
    auto next_dfa_state
            = [&dfa_states, &create_dfa_state](ConfigurationSet const& set) -> TypedDfaState* {
        TypedDfaState* state{nullptr};
        auto it = dfa_states.find(set);
        if (it == dfa_states.end()) {
            state = create_dfa_state(set);
        } else {
            state = it->second;
        }
        return state;
    };

    for (uint32_t i = 0; i < 4 * nfa.get_num_tags(); i++) {
        constexpr PrefixTree::position_t cDefaultPos{0};
        m_register_handler.add_register(PrefixTree::cRootId, cDefaultPos);
    }
    std::vector<uint32_t> initial_registers(nfa.get_num_tags() * 2);
    std::iota(initial_registers.begin(), initial_registers.end(), 0);

    DetermizationConfiguration<TypedNfaState>
            initial_configuration{nfa.get_root(), initial_registers, {}, {}};
    auto configuration_set = initial_configuration.epsilon_closure();
    create_dfa_state(configuration_set);
    while (false == unexplored_sets.empty()) {
        configuration_set = unexplored_sets.top();
        unexplored_sets.pop();

        auto* dfa_state = dfa_states.at(configuration_set);
        std::map<uint32_t, ConfigurationSet> ascii_transitions_map;
        for (auto const& configuration : configuration_set) {
            for (uint32_t i = 0; i < cSizeOfByte; i++) {
                auto const* nfa_state = configuration.get_state();
                for (auto const* next_nfa_state : nfa_state->get_byte_transitions(i)) {
                    DetermizationConfiguration<TypedNfaState> next_configuration{
                            next_nfa_state,
                            configuration.get_register_ids(),
                            configuration.get_sequence(),
                            {}
                    };
                    auto closure = next_configuration.epsilon_closure();
                    ascii_transitions_map[i].insert(closure.begin(), closure.end());
                }
            }
        }

        for (auto const& [ascii_value, configuration] : ascii_transitions_map) {
            auto* dest_state = next_dfa_state(configuration);
            dfa_state->add_byte_transition(ascii_value, dest_state);
        }
    }
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::new_state(ConfigurationSet const& configuration_set
) -> TypedDfaState* {
    m_states.emplace_back(std::make_unique<TypedDfaState>());
    auto* dfa_state = m_states.back().get();
    for (auto const configuration : configuration_set) {
        auto const* nfa_state = configuration.get_state();
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

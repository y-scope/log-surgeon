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
#include <log_surgeon/finite_automata/RegisterAction.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>

#include "DeterminizationConfiguration.hpp"

namespace log_surgeon::finite_automata {
template <typename TypedDfaState, typename TypedNfaState>
class Dfa {
public:
    explicit Dfa(Nfa<TypedNfaState> const& nfa);

    /**
     * @return A string representation of the DFA.
     */
    [[nodiscard]] auto serialize() const -> std::string;

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

    [[nodiscard]] auto get_tag_id_to_final_reg_id(
    ) const -> std::unordered_map<tag_id_t, register_id_t> {
        return m_tag_id_to_final_reg_id;
    }

private:
    using ConfigurationSet = std::set<DetermizationConfiguration<TypedNfaState>>;

    /**
     * Generate the DFA states from the given NFA using the superset determinization algorithm.
     * @oaram nfa The NFA used to generate the DFA.
     */
    auto generate(Nfa<TypedNfaState> const& nfa) -> void;

    /**
     * Iterates over the configurations in the closure to:
     * - Add the new registers needed to track the tags to 'm_register_handler'.
     * - Determine the operations to perform on the new registers.
     * @param num_tags The number of tags.
     * @param closure Returns the set of dfa configurations with updated `tag_to_reg_ids`.
     * @param tag_op_to_reg_id Returns the map with any new tag operations to register entries.
     * @returns The operations to perform on the new registers.
     */
    auto assign_transition_reg_ops(
            std::size_t num_tags,
            std::set<DetermizationConfiguration<TypedNfaState>>& closure,
            std::map<std::pair<tag_id_t, OperationSequence>, register_id_t>& tag_op_to_reg_id
    ) -> RegisterOperations;

    /**
     * Iterates over the configurations in the closure to:
     * - Add the new registers needed to track the tags to 'm_register_handler'.
     * - Determine the operations to perform on the new registers.
     * @param closure A set of dfa configurations.
     * @param num_tags The number of tags.
     * @param tag_op_to_reg_id Returns the map with any new tag operations to register entries.
     * @returns The operations to perform on the new registers.
     */
    auto check_if_state_maps(
            std::set<DetermizationConfiguration<TypedNfaState>> const& closure,
            std::size_t num_tags,
            std::map<std::pair<tag_id_t, OperationSequence>, register_id_t>& tag_op_to_reg_id
    ) -> RegisterOperations;

    /**
     * Creates a new DFA state based on a set of NFA states and adds it to `m_states`.
     * @param nfa_state_set The set of NFA states represented by this DFA state.
     * @return A pointer to the new DFA state.
     */
    auto new_state(ConfigurationSet const& nfa_state_set) -> TypedDfaState*;

    /**
     * @return A vector representing the traversal order of the DFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<TypedDfaState const*>;

    std::vector<std::unique_ptr<TypedDfaState>> m_states;
    std::unordered_map<tag_id_t, register_id_t> m_tag_id_to_final_reg_id;
    RegisterHandler m_register_handler;
    std::vector<RegisterAction> m_register_operations;
};

template <typename TypedDfaState, typename TypedNfaState>
Dfa<TypedDfaState, TypedNfaState>::Dfa(Nfa<TypedNfaState> const& nfa) {
    generate(nfa);
}

// TODO: handle utf8 case in DFA generation.
template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::generate(Nfa<TypedNfaState> const& nfa) -> void {
    std::map<ConfigurationSet, TypedDfaState*> dfa_states;
    std::queue<ConfigurationSet> unexplored_sets;
    auto create_or_get_dfa_state
            = [this, &dfa_states, &unexplored_sets](ConfigurationSet const& config_set
              ) -> TypedDfaState* {
        TypedDfaState* state;
        // TODO: Does this need to ignore history when checking if `config_set` is in `dfa_states`?
        auto it{dfa_states.find(config_set)};
        if (it == dfa_states.end()) {
            // TODO: Do we need to map to existing states? Use unit-tests to show this if so.
            state = new_state(config_set);
            dfa_states[config_set] = state;
            unexplored_sets.push(config_set);
        } else {
            state = it->second;
        }
        return state;
    };

    m_register_handler.add_registers(2 * nfa.get_num_tags());
    for (uint32_t i{0}; i < nfa.get_num_tags(); ++i) {
        m_tag_id_to_final_reg_id[i] = nfa.get_num_tags() + i;
    }

    std::unordered_map<tag_id_t, register_id_t> initial_tag_to_reg;
    for (uint32_t i{0}; i < nfa.get_num_tags(); i++) {
        initial_tag_to_reg[i] = i;
    }

    DetermizationConfiguration<TypedNfaState>
            initial_configuration{nfa.get_root(), initial_tag_to_reg, {}, {}};
    auto configuration_set{initial_configuration.spontaneous_closure()};
    create_or_get_dfa_state(configuration_set);
    while (false == unexplored_sets.empty()) {
        configuration_set = unexplored_sets.front();
        unexplored_sets.pop();
        auto* dfa_state{dfa_states.at(configuration_set)};
        std::map<std::pair<tag_id_t, OperationSequence>, register_id_t> tag_op_to_reg_id{};
        std::map<uint32_t, std::pair<RegisterOperations, ConfigurationSet>>
                ascii_transitions_map;
        for (auto const& configuration : configuration_set) {
            auto const* nfa_state{configuration.get_state()};
            for (uint32_t i{0}; i < cSizeOfByte; ++i) {
                for (auto const* next_nfa_state : nfa_state->get_byte_transitions(i)) {
                    DetermizationConfiguration<TypedNfaState> next_configuration{
                            next_nfa_state,
                            configuration.get_tag_to_reg_ids(),
                            configuration.get_sequence(),
                            {}
                    };
                    auto closure{next_configuration.spontaneous_closure()};
                    ascii_transitions_map[i].first = assign_transition_reg_ops(
                            nfa.get_num_tags(),
                            closure,
                            tag_op_to_reg_id
                    );
                    ascii_transitions_map[i].second.insert(closure.begin(), closure.end());
                }
            }
        }

        for (auto const& [ascii_value, config_pair] : ascii_transitions_map) {
            auto& [reg_ops, config_set]{config_pair};
            auto* dest_state{create_or_get_dfa_state(config_set)};
            dfa_state->add_byte_transition(ascii_value, reg_ops, dest_state);
        }
    }
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::assign_transition_reg_ops(
        std::size_t const num_tags,
        std::set<DetermizationConfiguration<TypedNfaState>>& closure,
        std::map<std::pair<tag_id_t, OperationSequence>, register_id_t>& tag_op_to_reg_id
) -> RegisterOperations {
    RegisterOperations register_operations;
    std::set<DetermizationConfiguration<TypedNfaState>> new_closure;
    for (auto config : closure) {
        for (tag_id_t tag_id{0}; tag_id < num_tags; tag_id++) {
            auto const new_register_operations{config.get_tag_history(tag_id)};
            if (false == new_register_operations.empty()) {
                auto const index{std::make_pair(tag_id, new_register_operations)};
                auto reg_id{tag_op_to_reg_id.find(index)};
                if (tag_op_to_reg_id.end() == reg_id) {
                    tag_op_to_reg_id[index] = m_register_handler.add_register();
                }
                register_operations.emplace(tag_op_to_reg_id[index], new_register_operations);
                config.set_reg_id(tag_id, tag_op_to_reg_id[index]);
            }
        }
        new_closure.insert(config);
    }
    closure = new_closure;
    return register_operations;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::check_if_state_maps(
        std::set<DetermizationConfiguration<TypedNfaState>> const& closure,
        std::size_t const num_tags,
        std::map<std::pair<tag_id_t, OperationSequence>, register_id_t>& tag_op_to_reg_id
) -> RegisterOperations {
    RegisterOperations register_operations;
    for (auto config : closure) {
        for (tag_id_t tag_id{0}; tag_id < num_tags; tag_id++) {
            auto const new_register_operations{config.get_tag_history(tag_id)};
            if (false == new_register_operations.empty()) {
                auto const index{std::make_pair(tag_id, new_register_operations)};
                auto reg_id{tag_op_to_reg_id.find(index)};
                if (tag_op_to_reg_id.end() == reg_id) {
                    tag_op_to_reg_id[index] = m_register_handler.add_register();
                }
                register_operations.emplace(tag_op_to_reg_id[index], new_register_operations);
                config.set_reg_id(tag_id, tag_op_to_reg_id[index]);
            }
        }
    };
    return register_operations;
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

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::get_bfs_traversal_order(
) const -> std::vector<TypedDfaState const*> {
    std::queue<TypedDfaState const*> state_queue;
    std::unordered_set<TypedDfaState const*> visited_states;
    std::vector<TypedDfaState const*> visited_order;
    visited_states.reserve(m_states.size());
    visited_order.reserve(m_states.size());

    auto add_to_queue_and_visited
            = [&state_queue, &visited_states](TypedDfaState const* dest_state) {
                  if (visited_states.insert(dest_state).second) {
                      state_queue.push(dest_state);
                  }
              };

    add_to_queue_and_visited(get_root());
    while (false == state_queue.empty()) {
        auto const* current_state = state_queue.front();
        visited_order.push_back(current_state);
        state_queue.pop();
        // TODO: handle the utf8 case
        for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
            auto const [register_operation, dest_state]{current_state->get_byte_transition(idx)};
            if (nullptr != dest_state) {
                add_to_queue_and_visited(dest_state);
            }
        }
    }
    return visited_order;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::serialize() const -> std::string {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<TypedDfaState const*, uint32_t> state_ids;
    for (auto const* state : traversal_order) {
        state_ids.emplace(state, state_ids.size());
    }

    std::vector<std::string> serialized_states;
    for (auto const* state : traversal_order) {
        serialized_states.emplace_back(state->serialize(state_ids));
    }
    return fmt::format("{}\n", fmt::join(serialized_states, "\n"));
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

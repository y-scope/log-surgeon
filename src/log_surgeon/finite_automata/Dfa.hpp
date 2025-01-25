#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_HPP

#include <cstddef>
#include <cstdint>
#include <map>
#include <memory>
#include <queue>
#include <set>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

#include <fmt/core.h>
#include <fmt/format.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/DeterminizationConfiguration.hpp>
#include <log_surgeon/finite_automata/DfaStatePair.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/Register.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>
#include <log_surgeon/finite_automata/RegisterOperation.hpp>
#include <log_surgeon/finite_automata/TagOperation.hpp>

namespace log_surgeon::finite_automata {
template <typename TypedDfaState, typename TypedNfaState>
class Dfa {
public:
    using ConfigurationSet = std::set<DetermizationConfiguration<TypedNfaState>>;

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
    /**
     * Generate the DFA states from the given NFA using the superset determinization algorithm.
     * @oaram nfa The NFA used to generate the DFA.
     */
    auto generate(Nfa<TypedNfaState> const& nfa) -> void;

    /**
     * Adds a register for tracking the initial and final value of each tag.
     * @param num_tags Number of tags in the NFA.
     * @param register_handler Returns the handler with the added registers.
     * @param initial_tag_id_to_reg_id Returns mapping of tag id to initial register id.
     * @param final_tag_id_to_reg_id Returns mapping of tag id to final register id.
     */
    static auto initialize_registers(
            size_t num_tags,
            RegisterHandler& register_handler,
            std::unordered_map<tag_id_t, register_id_t>& initial_tag_id_to_reg_id,
            std::unordered_map<tag_id_t, register_id_t>& final_tag_id_to_reg_id
    ) -> void;

    /**
     * Create a DFA state based on the given configuration set. If this is a new dfa state, its
     * added to the DFA's states, the config to state map, and the queue of unexplored states.
     * @param config_set The configuration set to create the state.
     * @param dfa_states Returns an updated map of configuration set to state.
     * @param unexplored_sets Returns a queue of unexplored states.
     * @return The existing or created dfa state.
     */
    auto create_or_get_dfa_state(
            ConfigurationSet const& config_set,
            std::map<ConfigurationSet, TypedDfaState*>& dfa_states,
            std::queue<ConfigurationSet>& unexplored_sets
    ) -> TypedDfaState*;

    /**
     * Determine the out-going transitions from the configuration set based on its NFA states.
     * @param config_set The configuration set.
     * @param tag_id_with_op_to_reg_id Returns an updated mapping from operation tag id to
     * register id.
     * @return Mapping of input to transition. Each transition contains a vector of register
     * operations and a destination configuration set.
     */
    auto get_transitions(
            ConfigurationSet const& config_set,
            std::unordered_map<tag_id_t, register_id_t>& tag_id_with_op_to_reg_id
    ) -> std::map<uint32_t, std::pair<std::vector<RegisterOperation>, ConfigurationSet>>;

    /**
     * Iterates over the configurations in the closure to:
     * - Add the new registers needed to track the tags to 'm_reg_handler'.
     * - Determine the operations to perform on the new registers.
     * @param closure Returns the set of dfa configurations with updated `tag_to_reg_ids`.
     * @param tag_id_with_op_to_reg_id Returns the updated map of tags with operations to
     * registers.
     * @returns The operations to perform on the new registers.
     */
    auto assign_transition_reg_ops(
            std::set<DetermizationConfiguration<TypedNfaState>>& closure,
            std::unordered_map<tag_id_t, register_id_t>& tag_id_with_op_to_reg_id
    ) -> std::vector<RegisterOperation>;

    /**
     * Creates a new DFA state based on a set of NFA configurations and adds it to `m_states`.
     * @param config_set The set of configurations represented by this DFA state.
     * @param tag_id_to_final_reg_id Mapping from tags to final reg
     * @return A pointer to the new DFA state.
     */
    auto new_state(
            ConfigurationSet const& config_set,
            std::unordered_map<tag_id_t, register_id_t> const& tag_id_to_final_reg_id
    ) -> TypedDfaState*;

    /**
     * @return A vector representing the traversal order of the DFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<TypedDfaState const*>;

    std::vector<std::unique_ptr<TypedDfaState>> m_states;
    std::unordered_map<tag_id_t, register_id_t> m_tag_id_to_final_reg_id;
    RegisterHandler m_reg_handler;
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

    std::unordered_map<tag_id_t, register_id_t> initial_tag_id_to_reg_id;
    initialize_registers(
            nfa.get_num_tags(),
            m_reg_handler,
            initial_tag_id_to_reg_id,
            m_tag_id_to_final_reg_id
    );

    DetermizationConfiguration<TypedNfaState>
            initial_configuration{nfa.get_root(), initial_tag_id_to_reg_id, {}, {}};
    auto config_set{initial_configuration.spontaneous_closure()};
    create_or_get_dfa_state(config_set, dfa_states, unexplored_sets);
    while (false == unexplored_sets.empty()) {
        config_set = unexplored_sets.front();
        unexplored_sets.pop();

        std::unordered_map<tag_id_t, register_id_t> tag_id_with_op_to_reg_id;
        for (auto const& [ascii_value, config_pair] :
             get_transitions(config_set, tag_id_with_op_to_reg_id))
        {
            auto& [reg_ops, config_set]{config_pair};
            auto* dest_state{create_or_get_dfa_state(config_set, dfa_states, unexplored_sets)};
            dfa_states.at(config_set)->add_byte_transition(ascii_value, {reg_ops, dest_state});
        }
    }
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::initialize_registers(
        size_t const num_tags,
        RegisterHandler& register_handler,
        std::unordered_map<tag_id_t, register_id_t>& initial_tag_id_to_reg_id,
        std::unordered_map<tag_id_t, register_id_t>& final_tag_id_to_reg_id
) -> void {
    register_handler.add_registers(2 * num_tags);
    for (uint32_t i{0}; i < num_tags; i++) {
        initial_tag_id_to_reg_id[i] = i;
        final_tag_id_to_reg_id[i] = num_tags + i;
    }
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::create_or_get_dfa_state(
        ConfigurationSet const& config_set,
        std::map<ConfigurationSet, TypedDfaState*>& dfa_states,
        std::queue<ConfigurationSet>& unexplored_sets
) -> TypedDfaState* {
    TypedDfaState* state{nullptr};
    // TODO: Does this need to ignore history when checking if `config_set` is in `dfa_states`?
    auto it{dfa_states.find(config_set)};
    if (it == dfa_states.end()) {
        // TODO: We need to map to existing states and do copy operations. Add a unit-test.
        state = new_state(config_set, m_tag_id_to_final_reg_id);
        dfa_states[config_set] = state;
        unexplored_sets.push(config_set);
    } else {
        state = it->second;
    }
    return state;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::get_transitions(
        ConfigurationSet const& config_set,
        std::unordered_map<tag_id_t, register_id_t>& tag_id_with_op_to_reg_id
) -> std::map<uint32_t, std::pair<std::vector<RegisterOperation>, ConfigurationSet>> {
    std::map<uint32_t, std::pair<std::vector<RegisterOperation>, ConfigurationSet>>
            ascii_transitions_map;
    for (auto const& configuration : config_set) {
        auto const* nfa_state{configuration.get_state()};
        for (uint32_t i{0}; i < cSizeOfByte; ++i) {
            for (auto const* next_nfa_state : nfa_state->get_byte_transitions(i)) {
                DetermizationConfiguration<TypedNfaState> next_configuration{
                        next_nfa_state,
                        configuration.get_tag_to_reg_ids(),
                        configuration.get_lookahead(),
                        {}
                };
                auto closure{next_configuration.spontaneous_closure()};
                auto const new_reg_ops{assign_transition_reg_ops(closure, tag_id_with_op_to_reg_id)
                };
                if (ascii_transitions_map.contains(i)) {
                    auto& byte_reg_ops{ascii_transitions_map.at(i).first};
                    byte_reg_ops.insert(byte_reg_ops.end(), new_reg_ops.begin(), new_reg_ops.end());
                    ascii_transitions_map.at(i).second.insert(closure.begin(), closure.end());
                } else {
                    ascii_transitions_map.emplace(i, std::make_pair(new_reg_ops, closure));
                }
            }
        }
    }
    return ascii_transitions_map;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::assign_transition_reg_ops(
        std::set<DetermizationConfiguration<TypedNfaState>>& closure,
        std::unordered_map<tag_id_t, register_id_t>& tag_id_with_op_to_reg_id
) -> std::vector<RegisterOperation> {
    std::vector<RegisterOperation> register_operations;
    std::set<DetermizationConfiguration<TypedNfaState>> new_closure;
    for (auto config : closure) {
        for (tag_id_t tag_id{0}; tag_id < tag_id_with_op_to_reg_id.size(); tag_id++) {
            auto const optional_tag_op{config.get_tag_history(tag_id)};
            if (optional_tag_op.has_value()) {
                auto const tag_op{optional_tag_op.value()};
                tag_id_with_op_to_reg_id.try_emplace(tag_id, m_reg_handler.add_register());
                if (TagOperationType::Set == tag_op.get_type()) {
                    register_operations.emplace_back(
                            tag_id_with_op_to_reg_id.at(tag_id),
                            RegisterOperationType::Set
                    );
                } else if (TagOperationType::Negate == tag_op.get_type()) {
                    register_operations.emplace_back(
                            tag_id_with_op_to_reg_id.at(tag_id),
                            RegisterOperationType::Negate
                    );
                }
                config.set_reg_id(tag_id, tag_id_with_op_to_reg_id.at(tag_id));
            }
        }
        new_closure.insert(config);
    }
    closure = new_closure;
    return register_operations;
}

template <typename TypedDfaState, typename TypedNfaState>
auto Dfa<TypedDfaState, TypedNfaState>::new_state(
        ConfigurationSet const& config_set,
        std::unordered_map<tag_id_t, register_id_t> const& tag_id_to_final_reg_id
) -> TypedDfaState* {
    m_states.emplace_back(std::make_unique<TypedDfaState>());
    auto* dfa_state = m_states.back().get();
    for (auto const configuration : config_set) {
        auto const* nfa_state = configuration.get_state();
        if (nfa_state->is_accepting()) {
            dfa_state->add_matching_variable_id(nfa_state->get_matching_variable_id());
            // TODO: Make operation sequence a single operation?
            // TODO: If sequence/lookahead exists for tag then do it to final reg for tag
            // - What happens if the current register for that tag is a sequence, don't we need
            //   to copy it?
            // Otherwise copy current register for tag into final reg
            for (tag_id_t tag_id{0}; tag_id < tag_id_to_final_reg_id.size(); tag_id++) {
                dfa_state->add_accepting_operation({tag_id, {RegisterOperationType::Copy}});
            }
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
            auto const transition{current_state->next(idx)};
            if (nullptr != transition.get_dest_state()) {
                add_to_queue_and_visited(transition.get_dest_state());
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

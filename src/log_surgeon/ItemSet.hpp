#ifndef LOG_SURGEON_ITEMSET_HPP
#define LOG_SURGEON_ITEMSET_HPP

#include <cstdint>
#include <set>
#include <tuple>
#include <unordered_map>
#include <vector>

#include <log_surgeon/Production.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {
/**
 * Structure representing an item in a LALR1 state.
 * An item (1) is associated with a m_production and a single m_lookahead which
 * is an input symbol (character) that can follow the m_production, and (2)
 * tracks the current matching progress of its associated m_production, where
 * everything exclusively to the left of m_dot is already matched.
 */
class Item {
public:
    Item() = default;

    Item(Production* p, uint32_t d, uint32_t t) : m_production(p), m_dot(d), m_lookahead(t) {}

    /**
     * Comparison operator for tie-breakers (not 100% sure where this is used)
     * @param lhs
     * @param rhs
     * @return bool
     */
    friend auto operator<(Item const& lhs, Item const& rhs) -> bool {
        return std::tie(lhs.m_production->m_index, lhs.m_dot, lhs.m_lookahead)
               < std::tie(rhs.m_production->m_index, rhs.m_dot, rhs.m_lookahead);
    }

    /**
     * Returns if the item has a dot at the end. This indicates the production
     * associated with the item has already been fully matched.
     * @return bool
     */
    [[nodiscard]] auto has_dot_at_end() const -> bool {
        return m_dot == m_production->m_body.size();
    }

    /**
     * Returns the next unmatched symbol in the production based on the dot.
     * @return uint32_t
     */
    [[nodiscard]] auto next_symbol() const -> uint32_t { return m_production->m_body.at(m_dot); }

    Production* m_production;
    uint32_t m_dot;
    uint32_t m_lookahead;  // for LR0 items, `m_lookahead` is unused
};

/**
 * Structure representing an LALR1 state, a collection of items.
 * The m_kernel is sufficient for fully representing the state, but m_closure is
 * useful for computations. m_next indicates what state (ItemSet) to transition
 * to based on the symbol received from the lexer m_actions is the action to
 * perform based on the symbol received from the lexer.
 */
class ItemSet {
public:
    /**
     * Comparison operator for tie-breakers (not 100% sure where this is used)
     * @param lhs
     * @param rhs
     * @return bool
     */
    friend auto operator<(ItemSet const& lhs, ItemSet const& rhs) -> bool {
        return lhs.m_kernel < rhs.m_kernel;
    }

    auto empty() const -> bool { return m_kernel.empty(); }

    uint32_t m_index = -1;
    std::set<Item> m_kernel;
    std::set<Item> m_closure;
    std::unordered_map<uint32_t, ItemSet*> m_next;
    std::vector<Action> m_actions;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_ITEMSET_HPP

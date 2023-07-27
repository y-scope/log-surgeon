#ifndef LOG_SURGEON_FINITE_AUTOMATA_UNICODE_INTERVAL_TREE_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_UNICODE_INTERVAL_TREE_HPP

#include <cstdint>
#include <memory>
#include <utility>
#include <vector>

namespace log_surgeon::finite_automata {
using Interval = std::pair<uint32_t, uint32_t>;

template <class T>
class UnicodeIntervalTree {
public:
    /**
     * Structure to represent utf8 data
     */
    struct Data {
    public:
        Data(Interval interval, T value) : m_interval(std::move(interval)), m_value(value) {}

        Interval m_interval;
        T m_value;
    };

    /**
     * Insert data into the tree
     * @param interval
     * @param value
     */
    auto insert(Interval interval, T value) -> void;

    /**
     * Returns all utf8 in the tree
     * @return std::vector<Data>
     */
    auto all() const -> std::vector<Data>;

    /**
     * Return an interval in the tree
     * @param interval
     * @return std::unique_ptr<std::vector<Data>>
     */
    auto find(Interval interval) -> std::unique_ptr<std::vector<Data>>;

    /**
     * Remove an interval from the tree
     * @param interval
     * @return std::unique_ptr<std::vector<Data>>
     */
    auto pop(Interval interval) -> std::unique_ptr<std::vector<Data>>;

    auto reset() -> void { m_root.reset(); }

private:
    class Node {
    public:
        // Constructor
        Node() = default;

        // Constructor
        Node(Interval i, T v) : m_interval(std::move(i)), m_value(v) {}

        /**
         * Balance the subtree below a node
         * @param node
         * @return std::unique_ptr<Node>
         */
        static auto balance(std::unique_ptr<Node> node) -> std::unique_ptr<Node>;

        /**
         * Insert a node
         * @param node
         * @param interval
         * @param value
         * @return std::unique_ptr<Node>
         */
        static auto insert(std::unique_ptr<Node> node, Interval interval, T value)
                -> std::unique_ptr<Node>;

        /**
         * Remove a node
         * @param node
         * @param interval
         * @param ret
         * @return std::unique_ptr<Node>
         */
        static auto pop(std::unique_ptr<Node> node, Interval interval, std::unique_ptr<Node>* ret)
                -> std::unique_ptr<Node>;

        /**
         * Remove a node
         * @param node
         * @param ret
         * @return std::unique_ptr<Node>
         */
        static auto pop_min(std::unique_ptr<Node> node, std::unique_ptr<Node>* ret)
                -> std::unique_ptr<Node>;

        /**
         * Rotate a node by a factor
         * @param node
         * @param factor
         * @return std::unique_ptr<Node>
         */
        static auto rotate(std::unique_ptr<Node> node, int factor) -> std::unique_ptr<Node>;

        /**
         * Rotate a node clockwise
         * @param node
         * @return std::unique_ptr<Node>
         */
        static auto rotate_cw(std::unique_ptr<Node> node) -> std::unique_ptr<Node>;

        /**
         * Rotate a node counterclockwise
         * @param node
         * @return std::unique_ptr<Node>
         */
        static auto rotate_ccw(std::unique_ptr<Node> node) -> std::unique_ptr<Node>;

        /**
         * add all utf8 in subtree to results
         * @param results
         */
        auto all(std::vector<Data>* results) -> void;

        /**
         * add all utf8 in subtree that matches interval to results
         * @param interval
         * @param results
         */
        auto find(Interval interval, std::vector<Data>* results) -> void;

        /**
         * update node
         */
        auto update() -> void;

        /**
         * get balance factor of node
         */
        auto balance_factor() -> int;

        /**
         * overlaps_recursive()
         * @param i
         */
        auto overlaps_recursive(Interval i) -> bool;

        /**
         * overlaps()
         * @param i
         */
        auto overlaps(Interval i) -> bool;

        auto get_interval() -> Interval { return m_interval; }

        auto get_value() -> T { return m_value; }

    private:
        Interval m_interval;
        T m_value;
        uint32_t m_lower{};
        uint32_t m_upper{};
        int m_height{};
        std::unique_ptr<Node> m_left;
        std::unique_ptr<Node> m_right;
    };

    std::unique_ptr<Node> m_root;
};
}  // namespace log_surgeon::finite_automata

#include "UnicodeIntervalTree.tpp"

#endif  // LOG_SURGEON_FINITE_AUTOMATA_UNICODE_INTERVAL_TREE_HPP

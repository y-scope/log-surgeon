#ifndef LOG_MECHANIC_HPP
#define LOG_MECHANIC_HPP

#include <type_traits>

namespace clp::log_mechanic {

	template<typename T> using Box = T*;

	template <typename T> struct is_rust_box_t: std::false_type {};
	template <typename T> struct is_rust_box_t<Box<T>>: std::true_type {};

	template<typename T> using Option = std::enable_if_t<is_rust_box_t<T>::value, T>;
}

#include "log_mechanic.generated.hpp"

#endif // LOG_MECHANIC_HPP

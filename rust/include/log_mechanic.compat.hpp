#include <type_traits>
#include <string_view>

namespace log_mechanic {

	template<typename T> using Box = T*;

	template<typename T> struct is_rust_box_t: std::false_type {};
	template<typename T> struct is_rust_box_t<Box<T>>: std::true_type {};
	template<typename T> constexpr bool is_rust_box_v = is_rust_box_t<T>::value;

	template<typename T> using Option = std::enable_if_t<is_rust_box_v<T>, T>;

	template<typename T> struct CArray {
		T const* pointer;
		size_t length;

		CArray() = default;

		CArray(T const* pointer, size_t length) : pointer(pointer), length(length) {}

		CArray(std::string_view const& view)
			requires std::is_same_v<T, char>
			: CArray { view.data(), view.size() }
		{}

		CArray(char const* c_str)
			requires std::is_same_v<T, char>
			: CArray { std::string_view { c_str } }
		{}

		operator std::string_view() const
			requires std::is_same_v<T, char>
		{
			return this->as_cpp_view();
		}

		std::string_view as_cpp_view() const
			requires std::is_same_v<T, char>
		{
			return { this->pointer, this->length };
		}
	};

	using CCharArray = CArray<char>;

	static_assert(std::is_trivial_v<CCharArray>);
	static_assert(std::is_standard_layout_v<CCharArray>);
}

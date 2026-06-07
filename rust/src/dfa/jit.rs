use cranelift::codegen::Context;
use cranelift::codegen::ir::AbiParam;
use cranelift::codegen::ir::Block;
use cranelift::codegen::ir::BlockArg;
use cranelift::codegen::ir::InstBuilder;
use cranelift::codegen::ir::MemFlags;
use cranelift::codegen::ir::Signature;
use cranelift::codegen::ir::Type;
use cranelift::codegen::ir::UserFuncName;
use cranelift::codegen::ir::Value;
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::types;
use cranelift::codegen::isa::Builder as IsaBuilder;
use cranelift::codegen::isa::OwnedTargetIsa;
use cranelift::codegen::settings;
use cranelift::codegen::settings::Builder as SettingsBuilder;
use cranelift::codegen::settings::Configurable;
use cranelift::codegen::settings::Flags;
use cranelift::frontend::FuncInstBuilder;
use cranelift::frontend::FunctionBuilder;
use cranelift::frontend::FunctionBuilderContext;
use cranelift::frontend::Switch;
use cranelift::frontend::Variable;
use cranelift_jit::JITBuilder;
use cranelift_jit::JITModule;
use cranelift_module::FuncId;
use cranelift_module::Linkage;
use cranelift_module::Module;
use cranelift_module::default_libcall_names;
use regex_syntax::utf8::Utf8Range;
use regex_syntax::utf8::Utf8Sequence;
use regex_syntax::utf8::Utf8Sequences;

use super::*;

pub type JittedDfa = extern "C" fn(*const u8, *const u8, char, *const *const u8) -> Option<RuleIdx>;

pub struct Jit {
	module: JITModule,
}

impl std::fmt::Debug for Jit {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_struct("Jit").finish()
	}
}

struct JitContext<'a> {
	func_builder: FunctionBuilder<'a>,
}

impl Jit {
	pub fn new() -> Self {
		let mut flag_builder: SettingsBuilder = settings::builder();

		flag_builder.set("use_colocated_libcalls", "false").unwrap();
		flag_builder.set("is_pic", "false").unwrap();
		flag_builder.set("opt_level", "none").unwrap();

		let isa_builder: IsaBuilder = cranelift_native::builder().unwrap_or_else(|msg| {
			panic!("host machine is not supported: {msg}");
		});

		let isa: OwnedTargetIsa = isa_builder.finish(Flags::new(flag_builder)).unwrap();

		let module: JITModule = JITModule::new(JITBuilder::with_isa(isa, default_libcall_names()));

		Self { module }
	}

	pub fn jit(&mut self, dfa: &Tdfa) -> Result<JittedDfa, ()> {
		now!(u1);
		let old_dfa: &Tdfa = dfa;
		let dfa: Tdfa = old_dfa.minimize();
		now!(t0);
		println!(
			"minimizing {} to {} took: {:?}",
			old_dfa.states.len(),
			dfa.states.len(),
			t0.duration_since(u1)
		);
		let module: &mut JITModule = &mut self.module;

		let mut ctx: Context = module.make_context();
		let mut func_ctx: FunctionBuilderContext = FunctionBuilderContext::new();

		let mut sig: Signature = module.make_signature();
		let ptr_ty: Type = module.isa().pointer_type();

		sig.params.push(AbiParam::new(ptr_ty)); // input_ptr
		sig.params.push(AbiParam::new(ptr_ty)); // input_ptr_end
		sig.params.push(AbiParam::new(types::I32)); // anchor
		sig.params.push(AbiParam::new(ptr_ty)); // new input ptr
		sig.returns.push(AbiParam::new(types::I16)); // rule

		let func: FuncId = module.declare_anonymous_function(&sig).unwrap();

		ctx.func.signature = sig;

		let mut func_builder: FunctionBuilder<'_> = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

		let entry: Block = func_builder.create_block();

		let mut states: Vec<Block> = Vec::new();
		states.resize_with(dfa.states.len(), || func_builder.create_block());
		let states: &[Block] = &states;

		func_builder.switch_to_block(entry);
		func_builder.append_block_params_for_function_params(entry);

		let zero8: Value = func_builder.ins().iconst(types::I8, 0);
		let zero16: Value = func_builder.ins().iconst(types::I16, 0);
		let zero32: Value = func_builder.ins().iconst(types::I32, 0);

		let input_ptr: Value = func_builder.block_params(entry)[0];
		let input_ptr_end: Value = func_builder.block_params(entry)[1];
		let anchor: Value = func_builder.block_params(entry)[2];
		let output: Value = func_builder.block_params(entry)[3];

		let current_state: Variable = func_builder.declare_var(types::I32);
		func_builder.def_var(current_state, zero32);

		let backup_rule_var: Variable = func_builder.declare_var(types::I16);
		func_builder.def_var(backup_rule_var, zero16);

		let backup_input_ptr_var: Variable = func_builder.declare_var(ptr_ty);
		let step_forward_input_ptr: Value = func_builder.ins().iadd_imm(input_ptr, 1);
		func_builder.def_var(backup_input_ptr_var, step_forward_input_ptr);

		let input_ptr_var: Variable = func_builder.declare_var(ptr_ty);
		func_builder.def_var(input_ptr_var, input_ptr);

		let current_ch_var: Variable = func_builder.declare_var(types::I32);
		func_builder.def_var(current_ch_var, anchor);

		func_builder.ins().jump(states[0], &[BlockArg::Value(input_ptr)]);

		let exit: Block = func_builder.create_block();
		{
			func_builder.switch_to_block(exit);

			let backup_rule: Value = func_builder.use_var(backup_rule_var);
			let backup_input_ptr: Value = func_builder.use_var(backup_input_ptr_var);
			let step_back_input_ptr: Value = func_builder.ins().iadd_imm(backup_input_ptr, -1);

			func_builder
				.ins()
				.store(MemFlags::new(), step_back_input_ptr, output, 0);

			func_builder.ins().return_(&[backup_rule]);
		}

		for (i, state) in dfa.states.iter().enumerate() {
			let block: Block = states[i];

			func_builder.append_block_param(block, ptr_ty); // input_ptr

			func_builder.switch_to_block(block);
			let input_ptr: Value = func_builder.block_params(block)[0];
			let (input_ch, input_ptr): (Value, Value) = if i > 0 {
				if let Some(rule_idx) = state.accepting_rule {
					let rule: Value = func_builder.ins().iconst(types::I16, i64::from(u16::from(rule_idx)));
					func_builder.def_var(backup_rule_var, rule);
					func_builder.def_var(backup_input_ptr_var, input_ptr);
				}

				decode_utf8_char(&mut func_builder, input_ptr, input_ptr_end, exit, ptr_ty)
			} else {
				(anchor, input_ptr)
			};

			transition2(&mut func_builder, &dfa.states[i], states, input_ptr, input_ch, exit);
			// transition(
			// 	&mut func_builder,
			// 	&dfa.states[i],
			// 	states,
			// 	input_ch,
			// 	input_ptr,
			// 	input_ptr_end,
			// 	exit,
			// );
		}

		func_builder.set_cold_block(exit);

		now!(t1);
		func_builder.seal_all_blocks();
		now!(t2);
		func_builder.finalize();
		now!(t3);

		module.define_function(func, &mut ctx).unwrap();
		now!(t4);
		// println!("func: {}", ctx.func.display());
		module.clear_context(&mut ctx);

		module.finalize_definitions().unwrap();
		now!(t5);

		let code: *const u8 = module.get_finalized_function(func);
		assert!(!code.is_null());
		now!(t6);

		println!(
			"jitted: {:?}",
			[
				t6.duration_since(t5),
				t5.duration_since(t4),
				t4.duration_since(t3),
				t3.duration_since(t2),
				t2.duration_since(t1),
				t1.duration_since(t0),
			]
		);

		let func: JittedDfa = unsafe { std::mem::transmute::<*const u8, JittedDfa>(code) };

		Ok(func)
	}
}

fn load_char(
	func_builder: &mut FunctionBuilder<'_>,
	current_input_ptr: Value,
	offset: i64,
	input_ptr_end: Value,
	fallback: Block,
) -> (Value, Value) {
	let block: Block = func_builder.create_block();

	let input_ptr: Value = func_builder.ins().iadd_imm(current_input_ptr, offset);
	let diff: Value = func_builder.ins().isub(input_ptr_end, input_ptr);

	let not_eof: Value = func_builder.ins().icmp_imm(IntCC::UnsignedGreaterThan, diff, 0);

	func_builder.ins().brif(not_eof, block, &[], fallback, &[]);
	func_builder.seal_block(block);
	func_builder.switch_to_block(block);

	let next_input_ch: Value = func_builder.ins().load(types::I8, MemFlags::new(), input_ptr, 0);
	let next_input_ptr: Value = func_builder.ins().iadd_imm(input_ptr, 1);

	(next_input_ch, next_input_ptr)
}

fn decode_utf8_char(
	func_builder: &mut FunctionBuilder<'_>,
	input_ptr: Value,
	input_ptr_end: Value,
	fallback: Block,
	ptr_ty: Type,
) -> (Value, Value) {
	let inaf_b: Block = func_builder.create_block();
	let otherwise_b: Block = func_builder.create_block();

	let end_b: Block = func_builder.create_block();
	func_builder.append_block_param(end_b, ptr_ty);
	func_builder.append_block_param(end_b, types::I32);

	let diff: Value = func_builder.ins().isub(input_ptr_end, input_ptr);

	let inaf_v: Value = func_builder.ins().icmp_imm(IntCC::UnsignedGreaterThan, diff, 3);
	func_builder.ins().brif(inaf_v, inaf_b, &[], otherwise_b, &[]);
	func_builder.seal_block(inaf_b);
	func_builder.seal_block(otherwise_b);

	{
		func_builder.switch_to_block(inaf_b);
		load_char32::<false>(func_builder, input_ptr, input_ptr_end, fallback, end_b);
	}
	{
		func_builder.switch_to_block(otherwise_b);
		load_char32::<true>(func_builder, input_ptr, input_ptr_end, fallback, end_b);
	}
	func_builder.seal_block(end_b);

	func_builder.switch_to_block(end_b);
	let params: &[Value] = func_builder.block_params(end_b);
	(params[1], params[0])
}

fn load_b<const WITH_BOUNDS_CHECK: bool>(
	func_builder: &mut FunctionBuilder<'_>,
	input_ptr: Value,
	input_ptr_end: Value,
	fallback: Block,
) -> (Value, Value) {
	if WITH_BOUNDS_CHECK {
		let not_eof_b: Block = func_builder.create_block();

		let diff: Value = func_builder.ins().isub(input_ptr_end, input_ptr);

		let not_eof_v: Value = func_builder.ins().icmp_imm(IntCC::UnsignedGreaterThan, diff, 0);
		func_builder.ins().brif(not_eof_v, not_eof_b, &[], fallback, &[]);
		func_builder.seal_block(not_eof_b);
		func_builder.switch_to_block(not_eof_b);
	}

	let input_ch: Value = func_builder.ins().load(types::I8, MemFlags::new(), input_ptr, 0);
	let input_ch: Value = func_builder.ins().uextend(types::I32, input_ch);
	let input_ptr: Value = func_builder.ins().iadd_imm(input_ptr, 1);

	(input_ptr, input_ch)
}

fn load_char32<const WITH_BOUNDS_CHECK: bool>(
	func_builder: &mut FunctionBuilder<'_>,
	input_ptr: Value,
	input_ptr_end: Value,
	fallback: Block,
	end_b: Block,
) {
	let multi_byte_b: Block = func_builder.create_block();
	let multi_byte_3_b: Block = func_builder.create_block();
	let multi_byte_4_b: Block = func_builder.create_block();

	let (input_ptr, ch_a): (Value, Value) =
		load_b::<WITH_BOUNDS_CHECK>(func_builder, input_ptr, input_ptr_end, fallback);

	let is_ascii_v: Value = func_builder.ins().icmp_imm(IntCC::UnsignedLessThan, ch_a, 0x80);

	func_builder.ins().brif(
		is_ascii_v,
		end_b,
		&[BlockArg::Value(input_ptr), BlockArg::Value(ch_a)],
		multi_byte_b,
		&[],
	);
	func_builder.seal_block(multi_byte_b);

	{
		func_builder.switch_to_block(multi_byte_b);

		let (input_ptr, ch_b): (Value, Value) =
			load_b::<WITH_BOUNDS_CHECK>(func_builder, input_ptr, input_ptr_end, fallback);

		let ch_b: Value = func_builder.ins().band_imm(ch_b, 0b0011_1111);

		let ch_a: Value = func_builder.ins().band_imm(ch_a, 0b0001_1111);
		let ch_a: Value = func_builder.ins().ishl_imm(ch_a, 6);

		let ch: Value = func_builder.ins().bor(ch_a, ch_b);

		let n: Value = func_builder.ins().ushr_imm(ch_a, 4);
		let n: Value = func_builder.ins().band_imm(n, 0b0000_0011);

		let t: Value = func_builder.ins().icmp_imm(IntCC::UnsignedLessThan, n, 2);

		func_builder.ins().brif(
			t,
			end_b,
			&[BlockArg::Value(input_ptr), BlockArg::Value(ch)],
			multi_byte_3_b,
			&[],
		);
		func_builder.seal_block(multi_byte_3_b);

		{
			func_builder.switch_to_block(multi_byte_3_b);

			let (input_ptr, ch_c): (Value, Value) =
				load_b::<WITH_BOUNDS_CHECK>(func_builder, input_ptr, input_ptr_end, fallback);

			let ch_c: Value = func_builder.ins().band_imm(ch_c, 0b0011_1111);

			let ch_b: Value = func_builder.ins().ishl_imm(ch_b, 6);

			let ch_a: Value = func_builder.ins().band_imm(ch_a, 0b0000_1111);
			let ch_a: Value = func_builder.ins().ishl_imm(ch_a, 12);

			let ch: Value = func_builder.ins().bor(ch_a, ch_b);
			let ch: Value = func_builder.ins().bor(ch, ch_c);

			let t: Value = func_builder.ins().icmp_imm(IntCC::UnsignedLessThan, n, 3);

			func_builder.ins().brif(
				t,
				end_b,
				&[BlockArg::Value(input_ptr), BlockArg::Value(ch)],
				multi_byte_4_b,
				&[],
			);
			func_builder.seal_block(multi_byte_4_b);

			{
				func_builder.switch_to_block(multi_byte_4_b);

				let (input_ptr, ch_d): (Value, Value) =
					load_b::<WITH_BOUNDS_CHECK>(func_builder, input_ptr, input_ptr_end, fallback);

				let ch_d: Value = func_builder.ins().band_imm(ch_d, 0b0011_1111);

				let ch_c: Value = func_builder.ins().ishl_imm(ch_c, 6);
				let ch_b: Value = func_builder.ins().ishl_imm(ch_b, 12);

				let ch_a: Value = func_builder.ins().band_imm(ch_a, 0b0000_0111);
				let ch_a: Value = func_builder.ins().ishl_imm(ch_a, 18);

				let ch: Value = func_builder.ins().bor(ch_a, ch_b);
				let ch: Value = func_builder.ins().bor(ch, ch_c);
				let ch: Value = func_builder.ins().bor(ch, ch_d);

				func_builder
					.ins()
					.jump(end_b, &[BlockArg::Value(input_ptr), BlockArg::Value(ch)]);
				// func_builder.seal_block(end_b);
			}
		}
	}

	// func_builder.switch_to_block(end_b);
	// let params: &[Value] = func_builder.block_params(end_b);
	// (params[0], params[1])
}

fn transition2(
	func_builder: &mut FunctionBuilder<'_>,
	current: &DfaState,
	states: &[Block],
	input_ptr: Value,
	input_ch: Value,
	fallback: Block,
) {
	let mut next: Block = func_builder.create_block();
	for (interval, transition) in current.transitions.iter() {
		let target: Block = states[transition.target];

		let start: Value = func_builder.ins().iconst(types::I32, i64::from(interval.start()));
		let end_offset: Value = func_builder
			.ins()
			.iconst(types::I32, i64::from(interval.end() - interval.start()));

		let x: Value = func_builder.ins().iadd_imm(input_ch, -i64::from(interval.start()));
		let t: Value = func_builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, x, end_offset);

		func_builder
			.ins()
			.brif(t, target, &[BlockArg::Value(input_ptr)], next, &[]);
		func_builder.seal_block(next);
		func_builder.switch_to_block(next);
		next = func_builder.create_block();
	}
	func_builder.ins().jump(fallback, &[]);
}

fn transition(
	func_builder: &mut FunctionBuilder<'_>,
	current: &DfaState,
	states: &[Block],
	input_ch: Value,
	input_ptr: Value,
	input_ptr_end: Value,
	fallback: Block,
) {
	let block1: Block = func_builder.create_block();

	func_builder.ins().jump(block1, &[]);
	func_builder.seal_block(block1);

	let mut switch: Switch = Switch::new();

	for (interval, transition) in current.transitions.iter() {
		let start: char = if let Some(ch) = char::try_from(interval.start()).ok() {
			ch
		} else if interval.start() == u32::from(char::MAX) + 1 {
			continue;
		} else if (0xD800..=0xDFFF).contains(&interval.start()) {
			'\u{E000}'
		} else {
			panic!("unexpected interval {interval:?}");
		};
		let end: char = char::try_from(interval.end()).ok().unwrap_or_else(|| {
			if interval.end() > u32::from(char::MAX) {
				char::MAX
			} else if (0xD800..=0xDFFF).contains(&interval.end()) {
				'\u{D799}'
			} else {
				panic!("unexpected interval {interval:?}");
			}
		});
		if start > end {
			continue;
		}
		for seq in Utf8Sequences::new(start, end) {
			let (first, rest): (&Utf8Range, &[Utf8Range]) = match &seq {
				Utf8Sequence::One(first) => (first, &[]),
				Utf8Sequence::Two([first, rest @ ..]) => (first, rest),
				Utf8Sequence::Three([first, rest @ ..]) => (first, rest),
				Utf8Sequence::Four([first, rest @ ..]) => (first, rest),
			};
			let full_range: bool = rest
				.iter()
				.all(|range| (range.start == 0) && (range.end == 0b1011_1111));

			let mut target: Block = states[transition.target];
			let block3: Block = func_builder.create_block();
			func_builder.switch_to_block(block3);
			let last_input_ptr: Value = func_builder
				.ins()
				.iadd_imm(input_ptr, i64::try_from(rest.len()).unwrap());
			func_builder.ins().jump(target, &[BlockArg::Value(last_input_ptr)]);
			target = block3;

			if full_range && false {
				let block2: Block = func_builder.create_block();
				func_builder.switch_to_block(block2);

				let last_input_ptr: Value = func_builder
					.ins()
					.iadd_imm(input_ptr, i64::try_from(rest.len()).unwrap());

				let diff: Value = func_builder.ins().isub(input_ptr_end, last_input_ptr);

				let not_eof: Value = func_builder.ins().icmp_imm(IntCC::UnsignedGreaterThan, diff, 0);

				func_builder.ins().brif(not_eof, target, &[], fallback, &[]);

				func_builder.seal_block(target);

				target = block2;
			} else {
				for (i, range) in rest.iter().enumerate().rev() {
					let block2: Block = func_builder.create_block();

					func_builder.switch_to_block(block2);

					let (next_input_ch, next_input_ptr): (Value, Value) = load_char(
						func_builder,
						input_ptr,
						i64::try_from(i).unwrap(),
						input_ptr_end,
						fallback,
					);

					/*
					let shifted_ch: Value = func_builder.ins().iadd_imm(next_input_ch, -i64::from(range.end));

					let in_range: Value = func_builder.ins().icmp_imm(
						IntCC::UnsignedGreaterThanOrEqual,
						shifted_ch,
						i64::from(range.end - range.start),
					);

					func_builder.ins().brif(in_range, target, &[], fallback, &[]);
					*/
					let mut switch2: Switch = Switch::new();
					for b in range.start..=range.end {
						switch2.set_entry(u128::from(b), target);
					}
					switch2.emit(func_builder, next_input_ch, fallback);
					func_builder.seal_block(target);

					target = block2;
				}
			}
			for b in first.start..=first.end {
				assert_eq!(switch.entries().get(&u128::from(b)), None);
				switch.set_entry(u128::from(b), target);
			}
			func_builder.seal_block(target);
		}
	}

	func_builder.switch_to_block(block1);
	switch.emit(func_builder, input_ch, fallback);
}

impl<'a> JitContext<'a> {
	fn ins<'short>(&'short mut self) -> FuncInstBuilder<'short, 'a> {
		self.func_builder.ins()
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn jit_test() {
		let mut jit: Jit = Jit::new();

		let schema = schema! {
			r#"
			delimiters: \ .
			int: [0-9]+$
			word: [a-z]+
			"#
		};

		assert_eq!(schema.delimiters, " .\n");

		let f = jit.jit(&schema.main_dfa).unwrap();

		let input: &[u8] = "abc 123 def 456".as_bytes();

		let mut end: *const u8 = std::ptr::null();

		let x: u16 = f(input.as_ptr_range().start, input.as_ptr_range().end, '\0', &mut end).map_or(0, u16::from);
		assert_eq!(x, 2);
		assert_eq!(end, input[.."abc".len()].as_ptr_range().end);

		let x: u16 = f(end, input.as_ptr_range().end, '\0', &mut end).map_or(0, u16::from);
		assert_eq!(x, 0);
		assert_eq!(end, input[.."abc".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			'\0',
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 1);
		assert_eq!(end, input[.."abc 123".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc 123 ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			'\0',
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 2);
		assert_eq!(end, input[.."abc 123 def".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc 123 def ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			'\0',
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 0);
		assert_eq!(end, input[.."abc 123 def ".len()].as_ptr_range().end);
	}
}

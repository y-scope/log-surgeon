use cranelift::codegen::Context;
use cranelift::codegen::ir::AbiParam;
use cranelift::codegen::ir::Block;
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

pub type JittedDfa = extern "C" fn(*const u8, *const u8, u8, *const *const u8) -> Option<RuleIdx>;

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
		flag_builder.set("opt_level", "speed").unwrap();

		let isa_builder: IsaBuilder = cranelift_native::builder().unwrap_or_else(|msg| {
			panic!("host machine is not supported: {msg}");
		});

		let isa: OwnedTargetIsa = isa_builder.finish(Flags::new(flag_builder)).unwrap();

		let module: JITModule = JITModule::new(JITBuilder::with_isa(isa, default_libcall_names()));

		Self { module }
	}

	pub fn jit(&mut self, dfa: &Tdfa) -> Result<JittedDfa, ()> {
		let module: &mut JITModule = &mut self.module;

		let mut ctx: Context = module.make_context();
		let mut func_ctx: FunctionBuilderContext = FunctionBuilderContext::new();

		let mut sig: Signature = module.make_signature();
		let ptr_ty: Type = module.isa().pointer_type();

		sig.params.push(AbiParam::new(ptr_ty)); // input_ptr
		sig.params.push(AbiParam::new(ptr_ty)); // input_ptr_end
		sig.params.push(AbiParam::new(types::I8)); // anchor
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

		let current_ch_var: Variable = func_builder.declare_var(types::I8);
		func_builder.def_var(current_ch_var, anchor);

		func_builder.ins().jump(states[0], &[]);

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
			func_builder.switch_to_block(states[i]);
			if i > 0 {
				if let Some(rule_idx) = state.accepting_rule {
					let rule: Value = func_builder.ins().iconst(types::I16, i64::from(u16::from(rule_idx)));
					func_builder.def_var(backup_rule_var, rule);
					let input_ptr: Value = func_builder.use_var(input_ptr_var);
					func_builder.def_var(backup_input_ptr_var, input_ptr);
				}

				load_char(&mut func_builder, current_ch_var, input_ptr_var, input_ptr_end, exit);
			}
			transition(
				&mut func_builder,
				&dfa.states[i],
				states,
				current_ch_var,
				input_ptr_var,
				input_ptr_end,
				exit,
			);
		}

		func_builder.seal_all_blocks();
		func_builder.finalize();

		module.define_function(func, &mut ctx).unwrap();
		module.clear_context(&mut ctx);

		module.finalize_definitions().unwrap();

		let code: *const u8 = module.get_finalized_function(func);
		assert!(!code.is_null());

		let func: JittedDfa = unsafe { std::mem::transmute::<*const u8, JittedDfa>(code) };

		Ok(func)
	}
}

fn enter_state(
	func_builder: &mut FunctionBuilder<'_>,
	current: &DfaState,
	states: &[Block],
	input_ptr_var: Variable,
	input_ptr_end: Value,
	fallback: Block,
	backup_input_ptr_var: Variable,
	backup_rule_var: Variable,
) {
	if let Some(rule_idx) = current.accepting_rule {
		let rule: Value = func_builder.ins().iconst(types::I32, i64::from(u16::from(rule_idx)));
		func_builder.def_var(backup_rule_var, rule);
		let input_ptr: Value = func_builder.use_var(input_ptr_var);
		func_builder.def_var(backup_input_ptr_var, input_ptr);
	}
}

fn load_char(
	func_builder: &mut FunctionBuilder<'_>,
	input_ch_var: Variable,
	input_ptr_var: Variable,
	input_ptr_end: Value,
	fallback: Block,
) -> Value {
	let block: Block = func_builder.create_block();

	let current_input_ch: Value = func_builder.use_var(input_ch_var);
	let current_input_ptr: Value = func_builder.use_var(input_ptr_var);

	let diff: Value = func_builder.ins().isub(input_ptr_end, current_input_ptr);

	let not_eof: Value = func_builder.ins().icmp_imm(IntCC::UnsignedGreaterThan, diff, 0);

	func_builder.ins().brif(not_eof, block, &[], fallback, &[]);
	func_builder.switch_to_block(block);

	let next_input_ch: Value = func_builder
		.ins()
		.load(types::I8, MemFlags::new(), current_input_ptr, 0);

	let next_input_ptr: Value = func_builder.ins().iadd_imm(current_input_ptr, 1);

	func_builder.def_var(input_ch_var, next_input_ch);
	func_builder.def_var(input_ptr_var, next_input_ptr);

	next_input_ch
}

fn transition(
	func_builder: &mut FunctionBuilder<'_>,
	current: &DfaState,
	states: &[Block],
	input_ch_var: Variable,
	input_ptr_var: Variable,
	input_ptr_end: Value,
	fallback: Block,
) {
	let mut block1: Block = func_builder.create_block();

	func_builder.ins().jump(block1, &[]);

	let mut switch: Switch = Switch::new();

	for (interval, transition) in current.transitions.iter() {
		let start: char = if let Some(ch) = char::try_from(interval.start()).ok() {
			ch
		} else if interval.start() == u32::from(char::MAX) + 1 {
			continue;
		} else {
			panic!("interval is {interval:?}, max is {:?}", char::MAX);
		};
		let end: char = char::try_from(interval.end()).ok().unwrap_or_else(|| {
			if interval.end() > u32::from(char::MAX) {
				char::MAX
			} else if (0xD800..=0xDFFF).contains(&interval.end()) {
				todo!();
			} else {
				panic!();
			}
		});
		let mut target: Block = states[transition.target];
		for seq in Utf8Sequences::new(start, end) {
			let (first, rest): (&Utf8Range, &[Utf8Range]) = match &seq {
				Utf8Sequence::One(first) => (first, &[]),
				Utf8Sequence::Two([first, rest @ ..]) => (first, rest),
				Utf8Sequence::Three([first, rest @ ..]) => (first, rest),
				Utf8Sequence::Four([first, rest @ ..]) => (first, rest),
			};
			for range in rest.iter().rev() {
				let block2: Block = func_builder.create_block();

				func_builder.switch_to_block(block2);

				let next_input_ch: Value =
					load_char(func_builder, input_ch_var, input_ptr_var, input_ptr_end, fallback);

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

				target = block2;
			}
			for b in first.start..=first.end {
				assert_eq!(switch.entries().get(&u128::from(b)), None);
				switch.set_entry(u128::from(b), target);
			}
		}
	}

	func_builder.switch_to_block(block1);
	let input_ch: Value = func_builder.use_var(input_ch_var);
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

		println!("- rule {:?}", schema.rules[0].regex);

		let f = jit.jit(&schema.main_dfa).unwrap();

		let input: &[u8] = "abc 123 def 456".as_bytes();

		let mut end: *const u8 = std::ptr::null();

		let x: u16 = f(input.as_ptr_range().start, input.as_ptr_range().end, 0, &mut end).map_or(0, u16::from);
		assert_eq!(x, 2);
		assert_eq!(end, input[.."abc".len()].as_ptr_range().end);

		let x: u16 = f(end, input.as_ptr_range().end, 0, &mut end).map_or(0, u16::from);
		assert_eq!(x, 0);
		assert_eq!(end, input[.."abc".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			0,
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 1);
		assert_eq!(end, input[.."abc 123".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc 123 ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			0,
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 2);
		assert_eq!(end, input[.."abc 123 def".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc 123 def ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			0,
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 0);
		assert_eq!(end, input[.."abc 123 def ".len()].as_ptr_range().end);
	}
}

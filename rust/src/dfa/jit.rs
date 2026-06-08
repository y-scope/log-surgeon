use cranelift::codegen::Context;
use cranelift::codegen::ir::AbiParam;
use cranelift::codegen::ir::Block;
use cranelift::codegen::ir::BlockArg;
use cranelift::codegen::ir::InstBuilder;
use cranelift::codegen::ir::MemFlags;
use cranelift::codegen::ir::Signature;
use cranelift::codegen::ir::Type;
// use cranelift::codegen::ir::UserFuncName;
use cranelift::codegen::ir::Value;
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::types;
use cranelift::codegen::isa::Builder as IsaBuilder;
use cranelift::codegen::isa::OwnedTargetIsa;
use cranelift::codegen::settings;
use cranelift::codegen::settings::Builder as SettingsBuilder;
use cranelift::codegen::settings::Configurable;
use cranelift::codegen::settings::Flags;
// use cranelift::frontend::FuncInstBuilder;
use cranelift::frontend::FunctionBuilder;
use cranelift::frontend::FunctionBuilderContext;
// use cranelift::frontend::Switch;
use cranelift::frontend::Variable;
use cranelift_jit::JITBuilder;
use cranelift_jit::JITModule;
use cranelift_module::FuncId;
// use cranelift_module::Linkage;
use cranelift_module::Module;
use cranelift_module::default_libcall_names;

use super::*;

pub type JittedDfa = extern "C" fn(*const u8, *const u8, u32, *const *const u8) -> Option<RuleIdx>;

pub struct Jit {
	module: JITModule,
	context: Context,
	function_context: FunctionBuilderContext,
}

impl std::fmt::Debug for Jit {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_struct("Jit").finish()
	}
}

struct Compilation<'ctx> {
	module: &'ctx mut JITModule,
	asm: FunctionBuilder<'ctx>,
	ptr_ty: Type,
}

impl Jit {
	pub fn new() -> Self {
		let mut flag_builder: SettingsBuilder = settings::builder();

		flag_builder.set("use_colocated_libcalls", "false").unwrap();
		flag_builder.set("is_pic", "false").unwrap();
		flag_builder.set("opt_level", "speed").unwrap();
		// flag_builder.set("opt_level", "none").unwrap();

		let isa_builder: IsaBuilder = cranelift_native::builder().unwrap_or_else(|msg| {
			panic!("host machine is not supported: {msg}");
		});

		let isa: OwnedTargetIsa = isa_builder.finish(Flags::new(flag_builder)).unwrap();

		let module: JITModule = JITModule::new(JITBuilder::with_isa(isa, default_libcall_names()));

		let context: Context = module.make_context();
		let function_context: FunctionBuilderContext = FunctionBuilderContext::new();

		Self {
			module,
			context,
			function_context,
		}
	}

	pub fn jit(&mut self, dfa: &Tdfa) -> Result<JittedDfa, ()> {
		// now!(u1);
		// let old_dfa: &Tdfa = dfa;
		// let dfa: Tdfa = old_dfa.minimize();
		now!(t0);
		// let mut ts: BTreeMap<usize, usize> = BTreeMap::new();
		// for s in dfa.states.iter() {
		// 	if s.accepting_rule.is_some() {
		// 		continue;
		// 	}
		// 	*ts.entry(s.transitions.len()).or_insert(0) += 1;
		// }
		// println!("dist: {ts:#?}");
		// println!(
		// 	"minimizing {} to {} took: {:?}",
		// 	old_dfa.states.len(),
		// 	dfa.states.len(),
		// 	t0.duration_since(u1)
		// );

		let mut sig: Signature = self.module.make_signature();
		let ptr_ty: Type = self.module.isa().pointer_type();

		sig.params.push(AbiParam::new(ptr_ty)); // input_ptr
		sig.params.push(AbiParam::new(ptr_ty)); // input_ptr_end
		sig.params.push(AbiParam::new(types::I32)); // anchor
		sig.params.push(AbiParam::new(ptr_ty)); // new input ptr
		sig.returns.push(AbiParam::new(types::I16)); // rule

		let func: FuncId = self.module.declare_anonymous_function(&sig).unwrap();

		self.context.func.signature = sig;

		(Compilation {
			module: &mut self.module,
			asm: FunctionBuilder::new(&mut self.context.func, &mut self.function_context),
			ptr_ty,
		})
		.compile(func, dfa);

		now!(t0);
		self.module.define_function(func, &mut self.context).unwrap();
		now!(t1);
		// println!("func: {}", ctx.func.display());
		self.module.clear_context(&mut self.context);

		self.module.finalize_definitions().unwrap();
		now!(t2);

		let code: *const u8 = self.module.get_finalized_function(func);
		assert!(!code.is_null());
		now!(t3);

		debug!(
			"jitted (define function, clear context and finalize definitions, get finalized): {:?}",
			[t1.duration_since(t0), t2.duration_since(t1), t3.duration_since(t2),]
		);

		let func: JittedDfa = unsafe { std::mem::transmute::<*const u8, JittedDfa>(code) };

		Ok(func)
	}
}

impl Compilation<'_> {
	fn compile(mut self, func: FuncId, dfa: &Tdfa) {
		now!(t0);
		let entry: Block = self.asm.create_block();

		let mut states: Vec<Block> = Vec::new();
		states.resize_with(dfa.states.len(), || self.asm.create_block());
		let states: &[Block] = &states;

		self.asm.switch_to_block(entry);
		self.asm.append_block_params_for_function_params(entry);

		let zero16: Value = self.asm.ins().iconst(types::I16, 0);
		let zero32: Value = self.asm.ins().iconst(types::I32, 0);

		let input_ptr: Value = self.asm.block_params(entry)[0];
		let input_ptr_end: Value = self.asm.block_params(entry)[1];
		let anchor: Value = self.asm.block_params(entry)[2];
		let output: Value = self.asm.block_params(entry)[3];

		let current_state: Variable = self.asm.declare_var(types::I32);
		self.asm.def_var(current_state, zero32);

		let backup_rule_var: Variable = self.asm.declare_var(types::I16);
		self.asm.def_var(backup_rule_var, zero16);

		let backup_input_ptr_var: Variable = self.asm.declare_var(self.ptr_ty);
		let step_forward_input_ptr: Value = self.asm.ins().iadd_imm(input_ptr, 1);
		self.asm.def_var(backup_input_ptr_var, step_forward_input_ptr);

		let input_ptr_var: Variable = self.asm.declare_var(self.ptr_ty);
		self.asm.def_var(input_ptr_var, input_ptr);

		let current_ch_var: Variable = self.asm.declare_var(types::I32);
		self.asm.def_var(current_ch_var, anchor);

		self.asm.ins().jump(states[0], &[BlockArg::Value(input_ptr)]);

		let mut count1: usize = 0;
		let mut count2: usize = 0;
		let exit: Block = self.asm.create_block();
		{
			self.asm.switch_to_block(exit);

			let backup_rule: Value = self.asm.use_var(backup_rule_var);
			let backup_input_ptr: Value = self.asm.use_var(backup_input_ptr_var);
			let step_back_input_ptr: Value = self.asm.ins().iadd_imm(backup_input_ptr, -1);

			self.asm.ins().store(MemFlags::new(), step_back_input_ptr, output, 0);

			self.asm.ins().return_(&[backup_rule]);
		}

		for (i, state) in dfa.states.iter().enumerate() {
			let block: Block = states[i];

			self.asm.append_block_param(block, self.ptr_ty); // input_ptr

			self.asm.switch_to_block(block);
			let input_ptr: Value = self.asm.block_params(block)[0];
			let (input_ch, input_ptr): (Value, Value) = if i > 0 {
				if let Some(rule_idx) = state.accepting_rule {
					let rule: Value = self.asm.ins().iconst(types::I16, i64::from(u16::from(rule_idx)));
					self.asm.def_var(backup_rule_var, rule);
					self.asm.def_var(backup_input_ptr_var, input_ptr);
				} else {
					assert_ne!(state.transitions.len(), 0);
				}

				self.next_char(input_ptr, input_ptr_end, exit, anchor)
			} else {
				(anchor, input_ptr)
			};

			self.do_transitions(
				&dfa.states[i], states, input_ptr, input_ch, exit, &mut count1, &mut count2,
			);
		}
		// println!("count1 {count1} count2 {count2}");

		self.asm.set_cold_block(exit);

		now!(t1);
		self.asm.seal_all_blocks();
		now!(t2);
		self.asm.finalize();
		now!(t3);

		debug!(
			"jitted (generate ir, seal all blocks, finalize ir): {:?}",
			[t1.duration_since(t0), t2.duration_since(t1), t3.duration_since(t2),]
		);
	}

	fn next_char(
		&mut self,
		input_ptr: Value,
		input_ptr_end: Value,
		fallback: Block,
		anchor_ch: Value,
	) -> (Value, Value) {
		let inaf_b: Block = self.asm.create_block();
		let otherwise_b: Block = self.asm.create_block();
		let anchor_b: Block = self.asm.create_block();

		let end_b: Block = self.asm.create_block();
		self.asm.append_block_param(end_b, self.ptr_ty);
		self.asm.append_block_param(end_b, types::I32);

		let diff: Value = self.asm.ins().isub(input_ptr_end, input_ptr);

		let inaf_v: Value = self.asm.ins().icmp_imm(IntCC::UnsignedGreaterThan, diff, 3);
		self.asm.ins().brif(inaf_v, inaf_b, &[], anchor_b, &[]);
		self.asm.seal_block(inaf_b);
		self.asm.seal_block(anchor_b);

		{
			self.asm.switch_to_block(inaf_b);
			self.decode_utf8_char::<false>(input_ptr, input_ptr_end, fallback, end_b);
		}
		{
			self.asm.switch_to_block(anchor_b);
			let at_end_v: Value = self.asm.ins().icmp_imm(IntCC::Equal, diff, 0);

			let input_ptr: Value = self.asm.ins().iadd_imm(input_ptr, 1);
			let eof_anchor: Value = self.asm.ins().iconst(types::I32, i64::from(b'\n'));

			self.asm.ins().brif(
				at_end_v,
				end_b,
				&[BlockArg::Value(input_ptr), BlockArg::Value(eof_anchor)],
				otherwise_b,
				&[],
			);
			self.asm.seal_block(otherwise_b);
		}
		{
			self.asm.switch_to_block(otherwise_b);
			self.decode_utf8_char::<true>(input_ptr, input_ptr_end, fallback, end_b);
		}
		self.asm.seal_block(end_b);

		self.asm.switch_to_block(end_b);
		let params: &[Value] = self.asm.block_params(end_b);
		(params[1], params[0])
	}

	fn decode_utf8_char<const WITH_BOUNDS_CHECK: bool>(
		&mut self,
		input_ptr: Value,
		input_ptr_end: Value,
		fallback: Block,
		end_b: Block,
	) {
		let multi_byte_b: Block = self.asm.create_block();
		let multi_byte_3_b: Block = self.asm.create_block();
		let multi_byte_4_b: Block = self.asm.create_block();

		let (input_ptr, ch_a): (Value, Value) = self.read_byte::<WITH_BOUNDS_CHECK>(input_ptr, input_ptr_end, fallback);

		let is_ascii_v: Value = self.asm.ins().icmp_imm(IntCC::UnsignedLessThan, ch_a, 0x80);

		self.asm.ins().brif(
			is_ascii_v,
			end_b,
			&[BlockArg::Value(input_ptr), BlockArg::Value(ch_a)],
			multi_byte_b,
			&[],
		);
		self.asm.seal_block(multi_byte_b);

		{
			self.asm.switch_to_block(multi_byte_b);

			let (input_ptr, ch_b): (Value, Value) =
				self.read_byte::<WITH_BOUNDS_CHECK>(input_ptr, input_ptr_end, fallback);

			let ch_b: Value = self.asm.ins().band_imm(ch_b, 0b0011_1111);

			let ch_a: Value = self.asm.ins().band_imm(ch_a, 0b0001_1111);
			let ch_a: Value = self.asm.ins().ishl_imm(ch_a, 6);

			let ch: Value = self.asm.ins().bor(ch_a, ch_b);

			let n: Value = self.asm.ins().ushr_imm(ch_a, 4);
			let n: Value = self.asm.ins().band_imm(n, 0b0000_0011);

			let t: Value = self.asm.ins().icmp_imm(IntCC::UnsignedLessThan, n, 2);

			self.asm.ins().brif(
				t,
				end_b,
				&[BlockArg::Value(input_ptr), BlockArg::Value(ch)],
				multi_byte_3_b,
				&[],
			);
			self.asm.seal_block(multi_byte_3_b);

			{
				self.asm.switch_to_block(multi_byte_3_b);

				let (input_ptr, ch_c): (Value, Value) =
					self.read_byte::<WITH_BOUNDS_CHECK>(input_ptr, input_ptr_end, fallback);

				let ch_c: Value = self.asm.ins().band_imm(ch_c, 0b0011_1111);

				let ch_b: Value = self.asm.ins().ishl_imm(ch_b, 6);

				let ch_a: Value = self.asm.ins().band_imm(ch_a, 0b0000_1111);
				let ch_a: Value = self.asm.ins().ishl_imm(ch_a, 12);

				let ch: Value = self.asm.ins().bor(ch_a, ch_b);
				let ch: Value = self.asm.ins().bor(ch, ch_c);

				let t: Value = self.asm.ins().icmp_imm(IntCC::UnsignedLessThan, n, 3);

				self.asm.ins().brif(
					t,
					end_b,
					&[BlockArg::Value(input_ptr), BlockArg::Value(ch)],
					multi_byte_4_b,
					&[],
				);
				self.asm.seal_block(multi_byte_4_b);

				{
					self.asm.switch_to_block(multi_byte_4_b);

					let (input_ptr, ch_d): (Value, Value) =
						self.read_byte::<WITH_BOUNDS_CHECK>(input_ptr, input_ptr_end, fallback);

					let ch_d: Value = self.asm.ins().band_imm(ch_d, 0b0011_1111);

					let ch_c: Value = self.asm.ins().ishl_imm(ch_c, 6);
					let ch_b: Value = self.asm.ins().ishl_imm(ch_b, 12);

					let ch_a: Value = self.asm.ins().band_imm(ch_a, 0b0000_0111);
					let ch_a: Value = self.asm.ins().ishl_imm(ch_a, 18);

					let ch: Value = self.asm.ins().bor(ch_a, ch_b);
					let ch: Value = self.asm.ins().bor(ch, ch_c);
					let ch: Value = self.asm.ins().bor(ch, ch_d);

					self.asm
						.ins()
						.jump(end_b, &[BlockArg::Value(input_ptr), BlockArg::Value(ch)]);
					// self.asm.seal_block(end_b);
				}
			}
		}

		// self.asm.switch_to_block(end_b);
		// let params: &[Value] = self.asm.block_params(end_b);
		// (params[0], params[1])
	}

	fn read_byte<const WITH_BOUNDS_CHECK: bool>(
		&mut self,
		input_ptr: Value,
		input_ptr_end: Value,
		fallback: Block,
	) -> (Value, Value) {
		if WITH_BOUNDS_CHECK {
			let not_eof_b: Block = self.asm.create_block();

			let diff: Value = self.asm.ins().isub(input_ptr_end, input_ptr);

			let not_eof_v: Value = self.asm.ins().icmp_imm(IntCC::SignedGreaterThan, diff, 0);
			self.asm.ins().brif(not_eof_v, not_eof_b, &[], fallback, &[]);
			self.asm.seal_block(not_eof_b);
			self.asm.switch_to_block(not_eof_b);
		}

		let input_ch: Value = self.asm.ins().load(types::I8, MemFlags::new(), input_ptr, 0);
		let input_ch: Value = self.asm.ins().uextend(types::I32, input_ch);
		let input_ptr: Value = self.asm.ins().iadd_imm(input_ptr, 1);

		(input_ptr, input_ch)
	}

	fn do_transitions(
		&mut self,
		current: &DfaState,
		states: &[Block],
		input_ptr: Value,
		input_ch: Value,
		fallback: Block,
		count1: &mut usize,
		count2: &mut usize,
	) {
		// let mut transitions1: Vec<(u32, u32, Block)> = Vec::new();
		let mut transitions2: Vec<(u32, u32, Block)> = Vec::new();
		// let mut map: BTreeMap<usize, Block> = BTreeMap::new();

		// let original: Block = self.asm.current_block().unwrap();

		for (interval, transition) in current.transitions.iter() {
			let target: Block = states[transition.target];
			// Intervals of length 4 or less are converted to a switch;
			// emprically, diminishing returns (fewer of them) of length greater than 4.
			// (if interval.end() - interval.start() < 4 {
			// 	target = *map.entry(transition.target).or_insert_with(|| {
			// 		let block: Block = self.asm.create_block();
			// 		self.asm.switch_to_block(block);
			// 		self.asm.ins().jump(target, &[BlockArg::Value(input_ptr)]);
			// 		block
			// 	});

			// 	&mut transitions1
			// } else {
			// 	&mut transitions2
			// })
			transitions2.push((interval.start(), interval.end(), target));
		}

		/*
		self.asm.switch_to_block(original);

		if !transitions1.is_empty() && transitions2.is_empty() {
			let next: Block = self.asm.create_block();
			let mut switch: Switch = Switch::new();

			for &(start, end, target) in transitions1.iter() {
				for ch in start..=end {
					switch.set_entry(u128::from(ch), target);
				}
			}

			switch.emit(self.asm, input_ch, next);
			self.asm.seal_block(next);
			self.asm.switch_to_block(next);
		}
		*/

		if !transitions2.is_empty() {
			/*
			let mut next: Block = self.asm.create_block();
			for &(start, end, target) in transitions2.iter() {
				let end_offset: Value = self.asm.ins().iconst(types::I32, i64::from(end - start));

				let x: Value = self.asm.ins().iadd_imm(input_ch, -i64::from(start));
				let t: Value = self.asm.ins().icmp(IntCC::UnsignedLessThanOrEqual, x, end_offset);

				self.asm
					.ins()
					.brif(t, target, &[BlockArg::Value(input_ptr)], next, &[]);
				self.asm.seal_block(next);
				self.asm.switch_to_block(next);
				next = self.asm.create_block();
			}
			*/
			*count2 += 1;
		} else {
			*count1 += 1;
		}
		self.binary_switch(input_ch, input_ptr, &transitions2, fallback);

		/*
		let mut next: Block = self.asm.create_block();
		for (interval, transition) in current.transitions.iter() {
			let target: Block = states[transition.target];

			let start: Value = self.asm.ins().iconst(types::I32, i64::from(interval.start()));
			let end_offset: Value = self.asm
				.ins()
				.iconst(types::I32, i64::from(interval.end() - interval.start()));

			let x: Value = self.asm.ins().iadd_imm(input_ch, -i64::from(interval.start()));
			let t: Value = self.asm.ins().icmp(IntCC::UnsignedLessThanOrEqual, x, end_offset);

			self.asm
				.ins()
				.brif(t, target, &[BlockArg::Value(input_ptr)], next, &[]);
			self.asm.seal_block(next);
			self.asm.switch_to_block(next);
			next = self.asm.create_block();
		}
		*/
		// self.asm.ins().jump(fallback, &[]);
	}

	fn binary_switch(&mut self, input_ch: Value, input_ptr: Value, intervals: &[(u32, u32, Block)], fallback: Block) {
		if intervals.is_empty() {
			self.asm.ins().jump(fallback, &[]);
			return;
		}

		let mid: usize = intervals.len() / 2;
		assert!(mid < intervals.len());
		let (left, right): (&[(u32, u32, Block)], &[(u32, u32, Block)]) = intervals.split_at(mid);
		let (lo, hi, target): (u32, u32, Block) = intervals[mid];

		if left.is_empty() {
			assert_eq!(right.len(), 1);
			let t: Value = self.asm.ins().iadd_imm(input_ch, -i64::from(lo));
			let t: Value = self
				.asm
				.ins()
				.icmp_imm(IntCC::UnsignedLessThanOrEqual, t, i64::from(hi - lo));

			self.asm
				.ins()
				.brif(t, target, &[BlockArg::Value(input_ptr)], fallback, &[]);
		} else {
			let block1: Block = self.asm.create_block();
			let block2: Block = self.asm.create_block();

			let t: Value = self
				.asm
				.ins()
				.icmp_imm(IntCC::UnsignedLessThan, input_ch, i64::from(lo));

			self.asm.ins().brif(t, block1, &[], block2, &[]);
			self.asm.seal_block(block1);
			self.asm.seal_block(block2);

			self.asm.switch_to_block(block1);
			self.binary_switch(input_ch, input_ptr, left, fallback);
			self.asm.switch_to_block(block2);
			self.binary_switch(input_ch, input_ptr, right, fallback);
		}
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

		let x: u16 = f(
			input.as_ptr_range().start,
			input.as_ptr_range().end,
			u32::from('\0'),
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 2);
		assert_eq!(end, input[.."abc".len()].as_ptr_range().end);

		let x: u16 = f(end, input.as_ptr_range().end, u32::from('\0'), &mut end).map_or(0, u16::from);
		assert_eq!(x, 0);
		assert_eq!(end, input[.."abc".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			u32::from('\0'),
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 1);
		assert_eq!(end, input[.."abc 123".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc 123 ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			u32::from('\0'),
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 2);
		assert_eq!(end, input[.."abc 123 def".len()].as_ptr_range().end);

		let x: u16 = f(
			input["abc 123 def ".len()..].as_ptr_range().start,
			input.as_ptr_range().end,
			u32::from('\0'),
			&mut end,
		)
		.map_or(0, u16::from);
		assert_eq!(x, 0);
		assert_eq!(end, input[.."abc 123 def ".len()].as_ptr_range().end);
	}
}

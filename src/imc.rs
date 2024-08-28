use crate::{
    options::Options,
    transys::{unroll::TransysUnroll, Transys},
};
use aig::Aig;
use cadical::{itp::Interpolant, Solver};
use logic_form::{Clause, Var};
use satif::Satif;
use std::collections::HashMap;

pub struct IMC {
    uts: TransysUnroll,
    options: Options,
}

impl IMC {
    pub fn new(options: Options, ts: Transys) -> Self {
        let uts = TransysUnroll::new(&ts);
        Self { uts, options }
    }

    pub fn check(&mut self) -> bool {
        for k in 0.. {
            let mut solver = cadical::Solver::new();
            let mut itp = Box::new(Interpolant::new());
            solver.connect_tracer(&itp);
            self.uts.unroll_to(k);
            for i in self.uts.ts.init.iter() {
                itp.label_clause(true);
                solver.add_clause(&[*i]);
            }
            for u in 0..=k {
                for c in self.uts.ts.trans.iter() {
                    let c: Clause = self.uts.lits_next(c, u);
                    itp.label_clause(u < k / 2);
                    solver.add_clause(&c);
                }
                for c in self.uts.ts.constraints.iter() {
                    let c = self.uts.lit_next(*c, u);
                    itp.label_clause(u < k / 2);
                    solver.add_clause(&[c]);
                }
            }
            if self.options.verbose > 0 {
                println!("bmc depth: {k}");
            }
            for b in self.uts.lits_next(&self.uts.ts.bad, k) {
                itp.label_clause(false);
                solver.add_clause(&[b]);
            }
            if solver.solve(&[]) {
                solver.disconnect_tracer(&itp);
                println!("bmc found cex in depth {k}");
                return true;
            } else {
                solver.disconnect_tracer(&itp);
                self.handle_itp(itp, k / 2);
            }
        }
        unreachable!();
    }

    pub fn handle_itp(&mut self, itp: Box<Interpolant>, m: usize) {
        let (itp, map) = itp.interpolant();
        let mut latch_map = HashMap::new();
        for l in self.uts.ts.latchs.iter() {
            latch_map.insert(self.uts.lit_next(l.lit(), m).var(), *l);
        }
        latch_map.insert(Var(0), Var(0));
        let mut remap = HashMap::new();
        for (k, v) in map {
            remap.insert(Var::new(v), latch_map[&k]);
        }
        self.check_inductive(itp, remap);
    }

    pub fn check_inductive(&mut self, itp: Aig, mut map: HashMap<Var, Var>) {
        println!("{itp}");
        let mut solver = Solver::new();
        self.uts.ts.load_init(&mut solver);
        self.uts.ts.load_trans(&mut solver, true);

        // let mut abc = Abc::new();
        // abc.read_aig(&itp);
        // abc.execute_command("fraig; dc2");
        // let itp = abc.write_aig();
        let mut cnf = itp.get_cnf(&itp.outputs);
        let mut vbegin = self.uts.ts.max_var + 1;
        for cls in cnf.iter_mut() {
            for l in cls.iter_mut() {
                if l.var() != Var(0) {
                    if let Some(v) = map.get(&l.var()) {
                        *l = v.lit().not_if(!l.polarity());
                    } else {
                        map.insert(l.var(), vbegin);
                        *l = vbegin.lit().not_if(!l.polarity());
                        vbegin += 1;
                    }
                }
            }
        }

        let itp_root = itp.outputs[0].to_lit();
        let itp_root = map
            .get(&itp_root.var())
            .unwrap()
            .lit()
            .not_if(!itp_root.polarity());

        for cls in cnf.iter() {
            solver.add_clause(cls);
        }

        if solver.solve(&[itp_root]) {
            panic!();
        }
    }
}

//! Standard-library behavior requiring VFS setup, exact diagnostics, or CPython comparison.

use std::process::Command;

use shellsim::Environment;

use super::support::{run_python as run, run_python_in as run_in};

#[test]
fn os_environ_supports_required_key_lookup() {
    let mut environment = Environment::new();
    environment.set_var("SHELLSIM_REQUIRED", "present");

    assert_eq!(
        run_in(
            &mut environment,
            "import os\nprint(os.environ['SHELLSIM_REQUIRED'])"
        ),
        (0, b"present\n".to_vec(), Vec::new())
    );
}

#[test]
fn print_accepts_common_output_keywords() {
    assert_eq!(
        run("print('a', 'b', sep=':', end='!')"),
        (0, b"a:b!".to_vec(), Vec::new())
    );
}

#[test]
fn os_chdir_changes_only_the_simulated_process_directory() {
    let mut environment = Environment::new();
    environment.set_var("PWD", "/work");
    environment.vfs.mkdir_all("/", "/work/project").unwrap();
    environment
        .vfs
        .put_file("/work/project/value.txt", b"inside\n".to_vec(), 0o644)
        .unwrap();
    let source = r#"import os
print(os.getcwd())
os.chdir("project")
print(os.getcwd())
print(open("value.txt").read().strip())
"#;
    assert_eq!(
        run_in(&mut environment, source),
        (0, b"/work\n/work/project\ninside\n".to_vec(), Vec::new())
    );
    assert_eq!(environment.cwd, "/work");
}

#[test]
fn os_listdir_and_walk_traverse_only_the_modeled_vfs() {
    let mut environment = Environment::new();
    environment.vfs.mkdir_all("/", "/work/tree/nested").unwrap();
    environment
        .vfs
        .put_file("/work/tree/root.txt", b"root".to_vec(), 0o644)
        .unwrap();
    environment
        .vfs
        .put_file("/work/tree/nested/leaf.txt", b"leaf".to_vec(), 0o644)
        .unwrap();
    environment
        .vfs
        .symlink("/", "/work/tree/nested", "/work/tree/link")
        .unwrap();
    let source = r#"import os
print(os.listdir("/work/tree"))
for root, directories, files in os.walk("/work/tree"):
    print(root, directories, files)
"#;
    assert_eq!(
        run_in(&mut environment, source),
        (
            0,
            b"['link', 'nested', 'root.txt']\n/work/tree ['link', 'nested'] ['root.txt']\n/work/tree/nested [] ['leaf.txt']\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn os_paths_accept_the_path_like_protocol() {
    let mut environment = Environment::new();
    environment.vfs.mkdir_all("/", "/work/tree").unwrap();
    environment
        .vfs
        .put_file("/work/tree/value.txt", b"value".to_vec(), 0o644)
        .unwrap();
    let source = r#"from pathlib import Path
import os
root = Path("/work/tree")
print(os.fspath(root), os.listdir(root))
print(list(os.walk(root)))
"#;
    assert_eq!(
        run_in(&mut environment, source),
        (
            0,
            b"/work/tree ['value.txt']\n[('/work/tree', [], ['value.txt'])]\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn vfs_operations_raise_the_python_os_exception_family() {
    let mut environment = Environment::new();
    environment.set_var("PWD", "/work");
    environment.vfs.put_dir("/work/folder", 0o755).unwrap();
    environment
        .vfs
        .put_file("/work/file", b"value".to_vec(), 0o644)
        .unwrap();
    let source = r#"import os
for operation in [
    lambda: open("missing").read(),
    lambda: open("folder").read(),
    lambda: os.chdir("file"),
]:
    try:
        operation()
    except FileNotFoundError:
        print("missing")
    except IsADirectoryError:
        print("directory")
    except NotADirectoryError:
        print("not-directory")
try:
    open("missing").read()
except OSError:
    print("os-base")
"#;
    assert_eq!(
        run_in(&mut environment, source),
        (
            0,
            b"missing\ndirectory\nnot-directory\nos-base\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn sys_exit_returns_the_requested_process_status() {
    assert_eq!(run("import sys\nsys.exit(4)"), (4, Vec::new(), Vec::new()));
    assert_eq!(run("import sys\nsys.exit()"), (0, Vec::new(), Vec::new()));
}

#[test]
fn sys_path_is_a_mutable_vfs_import_search_list() {
    let mut environment = Environment::new();
    environment.vfs.mkdir_all("/", "/opt/modules").unwrap();
    environment
        .vfs
        .put_file("/opt/modules/value.py", b"answer = 42\n".to_vec(), 0o644)
        .unwrap();
    let source = r#"import sys
sys.path.insert(0, "/opt/modules")
import value
print(sys.path[0], value.answer)
"#;
    assert_eq!(
        run_in(&mut environment, source),
        (0, b"/opt/modules 42\n".to_vec(), Vec::new())
    );
}

#[test]
fn math_and_string_constants_match_cpython() {
    let source = r#"import math
import string
prefix = string.ascii_lowercase[0] + string.ascii_lowercase[1] + string.ascii_lowercase[2]
print(math.sqrt(9), math.ceil(3 / 2), math.isinf(math.inf), prefix)
print(math.log(8, 2), math.sin(0))"#;
    let simulated = run(source);
    assert_eq!(
        simulated,
        (0, b"3.0 2 True abc\n3.0 0.0\n".to_vec(), Vec::new())
    );

    let reference = Command::new("python3.14").arg("-c").arg(source).output();
    if let Ok(reference) = reference {
        assert_eq!(simulated.0, reference.status.code().unwrap_or(1));
        assert_eq!(simulated.1, reference.stdout);
        assert_eq!(simulated.2, reference.stderr);
    }
}

#[test]
fn math_base_two_and_ten_logarithms_handle_values_and_errors() {
    let source = r#"import math
print(math.log2(8), math.log10(100))
print(math.log2(math.inf), math.isnan(math.log10(math.nan)))
for function in (math.log2, math.log10):
    for value in (0, -1):
        try:
            function(value)
        except ValueError as error:
            print("ValueError", str(error))
    try:
        function()
    except TypeError:
        print("TypeError")
"#;
    assert_eq!(
        run(source),
        (
            0,
            b"3.0 2.0\ninf True\nValueError math domain error\nValueError math domain error\nTypeError\nValueError math domain error\nValueError math domain error\nTypeError\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn math_comb_uses_exact_integers_and_rejects_invalid_arguments() {
    let source = r#"import math
n = 10 ** 20
print(math.comb(5, 2), math.comb(0, 0), math.comb(3, 4))
print(math.comb(n, 2) == n * (n - 1) // 2)
for left, right in ((-1, 0), (3, -1)):
    try:
        math.comb(left, right)
    except ValueError as error:
        print(str(error))
try:
    math.comb(3.0, 2)
except TypeError:
    print("TypeError")
"#;
    assert_eq!(
        run(source),
        (
            0,
            b"10 1 0\nTrue\nn must be a non-negative integer\nk must be a non-negative integer\nTypeError\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn expanded_math_and_complex_functions_cover_agent_numeric_checks() {
    let source = r#"import cmath
import math
print(math.radians(180), math.degrees(math.pi))
print(math.pow(2, 3), math.log10(100), math.hypot(3, 4))
print(round(math.atan(1), 6), round(math.atan2(1, 1), 6), round(math.asin(1), 6))
value = complex(1, 2)
print(value, value.real, value.imag, value.conjugate(), abs(complex(3, 4)))
print(cmath.sqrt(-1), cmath.exp(0), tuple(round(x, 6) for x in cmath.polar(complex(3, 4))))
print("radians" in dir(math), "sqrt" in dir(cmath))"#;
    assert_eq!(
        run(source),
        (
            0,
            b"3.141592653589793 180.0\n8.0 2.0 5.0\n0.785398 0.785398 1.570796\n(1+2j) 1.0 2.0 (1-2j) 5.0\n1j (1+0j) (5.0, 0.927295)\nTrue True\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn finite_itertools_bisect_and_integer_math_cover_common_recipes() {
    let source = r#"import bisect
import itertools
import math

values = [1, 3, 3, 7]
print(bisect.bisect_left(values, 3), bisect.bisect(values, 3))
bisect.insort(values, 4)
bisect.insort_left(values, 3)
print(values)
print(list(itertools.chain([1, 2], [3])))
print(list(itertools.product("ab", repeat=2)))
print(list(itertools.permutations([1, 2, 3], 2)))
print(list(itertools.combinations([1, 2, 3], 2)))
print(math.floor(-1.2), math.trunc(-1.8), math.fabs(-2))
print(math.isfinite(1.0), math.isfinite(math.inf))
print(math.gcd(18, 24), math.lcm(6, 8), math.factorial(6))
"#;
    assert_eq!(
        run(source),
        (
            0,
            b"1 3\n[1, 3, 3, 3, 4, 7]\n[1, 2, 3]\n[('a', 'a'), ('a', 'b'), ('b', 'a'), ('b', 'b')]\n[(1, 2), (1, 3), (2, 1), (2, 3), (3, 1), (3, 2)]\n[(1, 2), (1, 3), (2, 3)]\n-2 -1 2.0\nTrue False\n6 24 720\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn finite_iterator_heap_bisect_reduce_and_safe_imports_match_cpython() {
    let source = r#"import itertools
import heapq
import bisect
import functools
import typing
import subprocess

counter = itertools.count(3)
print(list(itertools.islice(counter, 3)))
values = [4, 1, 3]
heapq.heapify(values)
print(heapq.heappop(values), bisect.bisect_left([1, 3, 5], 4))
print(functools.reduce(lambda a, b: a + b, [1, 2, 3]))
plus_ten = functools.partial(lambda a, b: a + b, 10)
print(plus_ten(5), plus_ten.func(2, 3), plus_ten.args, plus_ten.keywords)
scaled = functools.partial(lambda value, scale=1: value * scale, scale=3)
print(scaled(4), scaled(4, scale=5), scaled.keywords)
print(typing.List[int])
print(next(counter))
"#;
    let simulated = run(source);
    let reference = Command::new("python3.14").arg("-c").arg(source).output();
    if let Ok(reference) = reference {
        assert_eq!(simulated.0, reference.status.code().unwrap_or(1));
        assert_eq!(simulated.1, reference.stdout);
        assert_eq!(simulated.2, reference.stderr);
    }
    assert_eq!(
        simulated,
        (
            0,
            b"[3, 4, 5]\n1 2\n6\n15 5 (10,) {}\n12 20 {'scale': 3}\ntyping.List[int]\n6\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn infinite_count_cannot_be_materialized_without_a_bound() {
    let simulated = run("import itertools\nprint(list(itertools.count()))");
    assert_ne!(simulated.0, 0);
    assert!(simulated.2.windows(8).any(|window| window == b"infinite"));
}

#[test]
fn regex_basic_operations_match_cpython() {
    let source = r##"import re
text = "A12 b34"
m = re.search(r"([A-Z])(\d+)", text)
print(m.group(0), m.group(1), m.group(2), m.start(), m.end())
print(re.match(r"[A-Z]", text).group())
print(re.findall(r"\d+", text))
print(re.sub(r"\d+", "#", text))
print(re.escape("a+b? c"))
print(re.search(r"b", text, flags=re.IGNORECASE).group())
pattern = re.compile(r"([a-z])(\d)")
matched = pattern.search("a1 b2")
print(matched.group(1), pattern.findall("a1 b2"), pattern.sub("#", "a1 b2"))
named = re.search(r"(?P<word>[a-z]+)-(?P<number>\d+)", "abc-42")
print(named.group("word"), named.group("number"), named.groups())
"##;
    let simulated = run(source);
    let reference = Command::new("python3.14").arg("-c").arg(source).output();
    if let Ok(reference) = reference {
        assert_eq!(simulated.0, reference.status.code().unwrap_or(1));
        assert_eq!(simulated.1, reference.stdout);
        assert_eq!(simulated.2, reference.stderr);
    }
    assert_eq!(
        simulated,
        (
            0,
            b"A12 A 12 0 3\nA\n['12', '34']\nA# b#\na\\+b\\?\\ c\nb\na [('a', '1'), ('b', '2')] # #\nabc 42 ('abc', '42')\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn argparse_parser_and_namespace_are_capability_free() {
    let source = r#"import argparse
parser = argparse.ArgumentParser(prog="demo")
parser.add_argument("--count", type=int, default=2)
parser.add_argument("--verbose", action="store_true")
parser.add_argument("--mode", choices=["fast", "safe"], default="safe")
args = parser.parse_args(["--count", "7", "--verbose", "--mode", "fast"])
print(parser.prog, args.count, args.verbose, args.mode)
"#;
    let simulated = run(source);
    assert_eq!(simulated, (0, b"demo 7 True fast\n".to_vec(), Vec::new()));
    let reference = Command::new("python3.14").arg("-c").arg(source).output();
    if let Ok(reference) = reference {
        assert_eq!(simulated.0, reference.status.code().unwrap_or(1));
        assert_eq!(simulated.1, reference.stdout);
        assert_eq!(simulated.2, reference.stderr);
    }

    let invalid = run(
        "import argparse\np = argparse.ArgumentParser()\np.add_argument('--mode', choices=['a'])\np.parse_args(['--mode', 'b'])",
    );
    assert_ne!(invalid.0, 0);
    assert!(invalid
        .2
        .windows(14)
        .any(|window| window == b"invalid choice"));
}

#[test]
fn argparse_help_known_args_and_subcommands_cover_common_cli_shapes() {
    let subcommands = run(r#"import argparse
parser = argparse.ArgumentParser(prog="tool", description="example tool")
commands = parser.add_subparsers(dest="command", required=True, help="available commands")
run_parser = commands.add_parser("run", help="run the job")
run_parser.add_argument("path", help="input path")
run_parser.add_argument("--count", type=int, default=2, help="repeat count")
run_parser.add_argument("--enabled", action="store_false")
args = parser.parse_args(["run", "input.txt", "--count", "4", "--enabled"])
print(args.command, args.path, args.count, args.enabled)
"#);
    assert_eq!(
        subcommands,
        (0, b"run input.txt 4 False\n".to_vec(), Vec::new())
    );

    let known = run(r#"import argparse
parser = argparse.ArgumentParser(add_help=False)
parser.add_argument("--seed", type=int, default=1)
args, rest = parser.parse_known_args(["--seed", "7", "--foreign", "value"])
print(args.seed, rest)
"#);
    assert_eq!(
        known,
        (0, b"7 ['--foreign', 'value']\n".to_vec(), Vec::new())
    );

    let help = run(r#"import argparse
parser = argparse.ArgumentParser(prog="tool", description="example tool")
parser.add_argument("--count", type=int, help="repeat count")
parser.parse_args(["--help"])
"#);
    assert_eq!(help.0, 0);
    let help = String::from_utf8(help.1).unwrap();
    assert!(help.contains("usage: tool [-h] [--count COUNT]"));
    assert!(help.contains("example tool"));
    assert!(help.contains("-h, --help"));
    assert!(help.contains("--count COUNT"));
    assert!(help.contains("repeat count"));
}

#[test]
fn importlib_util_executes_vfs_source_in_an_isolated_module() {
    let mut environment = Environment::new();
    environment.vfs.mkdir_all("/", "/work/pkg").unwrap();
    environment
        .vfs
        .put_file(
            "/work/pkg/helper.py",
            b"def double(value):\n    return value * 2\n".to_vec(),
            0o644,
        )
        .unwrap();
    environment
        .vfs
        .put_file(
            "/work/pkg/plugin.py",
            b"from helper import double\nvalue = double(21)\n".to_vec(),
            0o644,
        )
        .unwrap();
    let source = r#"import importlib.util
spec = importlib.util.spec_from_file_location("plugin_name", "/work/pkg/plugin.py")
module = importlib.util.module_from_spec(spec)
print(importlib.__name__, importlib.util.__name__)
print(spec.name, spec.origin, spec.loader is not None)
spec.loader.exec_module(module)
print(module.__name__, module.__file__, module.__package__, module.__spec__ is spec, module.value)
"#;
    assert_eq!(
        run_in(&mut environment, source),
        (
            0,
            b"importlib importlib.util\nplugin_name /work/pkg/plugin.py True\nplugin_name /work/pkg/plugin.py  True 42\n".to_vec(),
            Vec::new()
        )
    );
}

#[test]
fn regex_frontier_constructs_fail_closed() {
    for source in [
        r#"import re
print(re.search(r"(?=a)", "a"))
"#,
        r#"import re
print(re.search(r"(a)\1", "aa"))
"#,
    ] {
        let simulated = run(source);
        assert_ne!(simulated.0, 0, "unsupported regex syntax was accepted");
        assert!(
            !simulated.2.is_empty(),
            "unsupported regex syntax failed silently"
        );
    }
}

#[test]
fn hyperbolic_math_functions_match_cpython_within_one_ulp() {
    let source = r#"import math
print(math.sinh(1), math.cosh(1), math.tanh(1))
print(math.asinh(1), math.acosh(2), math.atanh(0.5))"#;
    let (status, stdout, stderr) = run(source);
    assert_eq!(status, 0);
    assert!(stderr.is_empty());
    let values = String::from_utf8(stdout)
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    // References recorded from CPython 3.14 on macOS. Both runtimes use platform libm;
    // Linux atanh(0.5) returns the adjacent float ending in 0548 rather than 0549.
    // Check numeric accuracy, not platform-dependent last digits in float repr.
    let expected: [f64; 6] = [
        1.1752011936438014,
        1.5430806348152437,
        0.7615941559557649,
        0.881373587019543,
        1.3169578969248166,
        0.5493061443340549,
    ];
    assert_eq!(values.len(), expected.len());
    for (value, expected) in values.into_iter().zip(expected) {
        assert!(value.is_finite());
        // All reference values are positive, so bit distance is the ULP distance.
        assert!(
            value.to_bits().abs_diff(expected.to_bits()) <= 1,
            "{value} differs from {expected} by more than one ULP"
        );
    }
}

#[test]
fn hyperbolic_functions_preserve_special_values_and_report_domain_errors() {
    let source = r#"import math
for name in ['sinh', 'cosh', 'tanh', 'asinh', 'acosh', 'atanh']:
    f = getattr(math, name)
    assert math.isnan(f(math.nan))
    try:
        f(1j)
    except TypeError:
        pass
    else:
        assert False
assert math.sinh(math.inf) == math.inf
assert math.sinh(-math.inf) == -math.inf
assert math.cosh(-math.inf) == math.inf
assert math.tanh(-math.inf) == -1.0
assert math.asinh(-math.inf) == -math.inf
assert math.acosh(math.inf) == math.inf
assert str(math.sinh(-0.0)) == '-0.0'
for f, x in [(math.acosh, 0), (math.atanh, 1), (math.atanh, -1), (math.atanh, math.inf)]:
    try:
        f(x)
    except ValueError as error:
        print(error)
for f in [math.sinh, math.cosh]:
    try:
        f(1000)
    except OverflowError as error:
        print(error)
try:
    math.sinh(1, 2)
except TypeError:
    print('arity')
"#;
    assert_eq!(run(source), (0, b"math domain error\nmath domain error\nmath domain error\nmath domain error\nmath range error\nmath range error\narity\n".to_vec(), Vec::new()));
}

#[test]
fn math_gamma_functions_match_cpython() {
    let (status, stdout, stderr) = run("import math\nprint(math.gamma(5), math.lgamma(5))");
    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert!(stderr.is_empty());
    let values = String::from_utf8(stdout)
        .unwrap()
        .split_whitespace()
        .map(|text| text.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], 24.0);
    assert!((values[1] - 3.1780538303479444).abs() <= 2.0 * f64::EPSILON * 3.1780538303479444);
}

#[test]
fn fractions_module_supports_exact_rational_arithmetic() {
    let source = r#"from fractions import Fraction
print(Fraction(1, 3) + Fraction(1, 6))
print(Fraction(2, 4))
print(Fraction(1, 3) * 3)"#;
    assert_eq!(run(source), (0, b"1/2\n1/2\n1\n".to_vec(), Vec::new()));
}

#[test]
fn gamma_domains_infinities_and_range_errors_match_python() {
    let source = r#"import math
assert abs(math.gamma(0.5) - math.sqrt(math.pi)) < 1e-14
assert abs(math.gamma(-0.5) + 2 * math.sqrt(math.pi)) < 1e-14
assert abs(math.lgamma(-0.5) - math.log(2 * math.sqrt(math.pi))) < 1e-14
assert math.gamma(math.inf) == math.inf
assert math.lgamma(-math.inf) == math.inf
assert math.isnan(math.gamma(math.nan))
assert math.isnan(math.lgamma(math.nan))
for fn in [math.gamma, math.lgamma]:
    for x in [0.0, -0.0, -1.0, -2.0]:
        try:
            fn(x)
            assert False
        except ValueError:
            pass
    try:
        fn(1, 2)
        assert False
    except TypeError:
        pass
try:
    math.gamma(-math.inf)
    assert False
except ValueError:
    pass
for fn, x in [(math.gamma, 172.0), (math.lgamma, 1e308)]:
    try:
        fn(x)
        assert False
    except OverflowError:
        pass
print('ok')
"#;
    assert_eq!(run(source), (0, b"ok\n".to_vec(), Vec::new()));
}

#[test]
fn fractions_normalize_construct_compare_and_mix_with_numbers() {
    // Expected values checked against CPython 3.14's fractions module.
    let source = r#"from fractions import Fraction as F
assert F() == 0
assert str(F(2, -4)) == '-1/2'
assert repr(F(2, 4)) == 'Fraction(1, 2)'
assert F(F(1, 3), F(2, 3)) == F(1, 2)
assert F(' -1.25e-2 ') == F(-1, 80)
assert F('1_000 / 2') == 500
assert F('.125') == F(1, 8)
assert F(0.1).as_integer_ratio() == (3602879701896397, 36028797018963968)
assert F(1e100).denominator == 1
assert F(5e-324).denominator == 2**1074
assert F(0.1) != F(1, 10)
assert F(1, 3) < 0.5 and 0.5 > F(1, 3)
assert F(1, 2) == 0.5
assert F(10**1000) < float('inf')
assert F(-10**1000) > -float('inf')
assert not F(1, 2) <= float('nan')
assert not F(0) and bool(F(1, 3))
assert F(2, 3) - F(1, 6) == F(1, 2)
assert 1 - F(1, 3) == F(2, 3)
assert 3 * F(1, 3) == 1
assert F(1, 3) / 2 == F(1, 6)
assert 2 / F(1, 3) == 6
assert F(-7, 3) // 2 == -2
assert F(-7, 3) % 2 == F(5, 3)
assert F(2, 3)**-2 == F(9, 4)
assert abs(-F(1, 3)) == F(1, 3)
assert int(F(-7, 3)) == -2
assert float(F(1, 2)) == 0.5
assert F(1, 2) + 0.25 == 0.75
assert F('3.1415926535897932').limit_denominator(1000) == F(355, 113)
for value in ['1__2', '1/2/3', 'nan', '1/-2', '1.2.3', '']:
    try:
        F(value)
        assert False
    except ValueError:
        pass
for args in [(1, 0), (0, 0)]:
    try:
        F(*args)
        assert False
    except ZeroDivisionError:
        pass
try:
    F(1.5, 2)
    assert False
except TypeError:
    pass
try:
    F(1, 2).numerator = 3
    assert False
except AttributeError:
    pass
print('ok')
"#;
    assert_eq!(run(source), (0, b"ok\n".to_vec(), Vec::new()));
}

#[test]
fn fraction_arithmetic_remains_resource_bounded() {
    let mut environment = Environment::with_limits(shellsim::Limits {
        cpu: 1_000_000,
        memory: 256 * 1024,
        disk: 1024 * 1024,
        output: 1024,
    });
    let source = "from fractions import Fraction\nx = Fraction(1, 3)\nprint('ready')\nwhile True:\n    x = x * x";
    let (status, stdout, stderr) = run_in(&mut environment, source);
    assert_eq!(stdout, b"ready\n");
    assert_eq!(status, 137);
    assert!(stderr.is_empty());
}

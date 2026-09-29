<?php
// PLAN §2.1: 1M method calls, property reads/writes, __get magic, interface
// dispatch, closures with bound $this, exceptions thrown/caught 100k times.
require __DIR__ . '/lib/harness.php';

const N = 1000000;

interface Shape { public function area(): float; }
final class Rect implements Shape {
    public function __construct(private float $w, private float $h) {}
    public function area(): float { return $this->w * $this->h; }
}
final class Circle implements Shape {
    public function __construct(private float $r) {}
    public function area(): float { return 3.0 * $this->r * $this->r; }
}
final class Tri implements Shape {
    public function __construct(private float $b, private float $h) {}
    public function area(): float { return 0.5 * $this->b * $this->h; }
}

class Counter {
    public int $n = 0;
    protected int $p = 0;
    private array $data = ['x' => 1, 'y' => 2];
    public static int $s = 0;
    public function inc(): void { $this->n++; }
    public function add(int $a, int $b): int { return $a + $b + $this->n; }
    public function getP(): int { return $this->p; }
    public function setP(int $v): static { $this->p = $v; return $this; }
    public static function sinc(): int { return ++self::$s; }
    public function __get($name) { return $this->data[$name] ?? null; }
    public function __set($name, $v) { $this->data[$name] = $v; }
    public function __call($name, $args) { return $args[0] + 1; }
    public function adder(): Closure { return function (int $x) { return $x + $this->n; }; }
}
class SubCounter extends Counter {
    public function add(int $a, int $b): int { return parent::add($a, $b) + 1; }
}
class AppException extends RuntimeException {}

function plain_add(int $a, int $b): int { return $a + $b; }

bench('function_call_1m', function () {
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += plain_add($i, 1); }
    return $s;
});

bench('method_call_1m', function () {
    $c = new Counter();
    for ($i = 0; $i < N; $i++) { $c->inc(); }
    return $c->n;
});

bench('method_call_args_ret_1m', function () {
    $c = new Counter();
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $c->add($i, 2); }
    return $s;
});

bench('parent_call_1m', function () {
    $c = new SubCounter();
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $c->add($i, 2); }
    return $s;
});

bench('static_method_call_1m', function () {
    Counter::$s = 0;
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += Counter::sinc(); }
    return $s;
});

bench('prop_read_1m', function () {
    $c = new Counter();
    $c->n = 3;
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $c->n; }
    return $s;
});

bench('prop_write_1m', function () {
    $c = new Counter();
    for ($i = 0; $i < N; $i++) { $c->n = $i; }
    return $c->n;
});

bench('prop_rmw_1m', function () {
    $c = new Counter();
    for ($i = 0; $i < N; $i++) { $c->n += 2; }
    return $c->n;
});

bench('getter_setter_fluent_1m', function () {
    $c = new Counter();
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $c->setP($i)->getP(); }
    return $s;
});

bench('magic_get_1m', function () {
    $c = new Counter();
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $c->x + $c->y; }
    return $s;
});

bench('magic_set_500k', function () {
    $c = new Counter();
    for ($i = 0; $i < 500000; $i++) { $c->z = $i; }
    return $c->z;
});

bench('magic_call_500k', function () {
    $c = new Counter();
    $s = 0;
    for ($i = 0; $i < 500000; $i++) { $s += $c->whatever($i); }
    return $s;
});

bench('interface_dispatch_3way_1m', function () {
    $shapes = [new Rect(2, 3), new Circle(1.5), new Tri(4, 5)];
    $s = 0.0;
    for ($i = 0; $i < N; $i++) { $s += $shapes[$i % 3]->area(); }
    return $s;
});

bench('instanceof_1m', function () {
    $shapes = [new Rect(2, 3), new Circle(1.5), new Tri(4, 5)];
    $n = 0;
    for ($i = 0; $i < N; $i++) {
        $o = $shapes[$i % 3];
        if ($o instanceof Shape) { $n++; }
        if ($o instanceof Circle) { $n++; }
    }
    return $n;
});

bench('new_object_1m', function () {
    $s = 0.0;
    for ($i = 0; $i < N; $i++) { $r = new Rect($i, 2); $s += $r->area(); }
    return $s;
});

bench('closure_bound_this_1m', function () {
    $c = new Counter();
    $c->n = 5;
    $f = $c->adder();
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $f($i); }
    return $s;
});

bench('closure_use_1m', function () {
    $k = 7;
    $f = function (int $x) use ($k) { return $x + $k; };
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $f($i); }
    return $s;
});

bench('arrow_fn_1m', function () {
    $k = 7;
    $f = fn(int $x) => $x + $k;
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $f($i); }
    return $s;
});

bench('first_class_callable_1m', function () {
    $c = new Counter();
    $f = $c->add(...);
    $s = 0;
    for ($i = 0; $i < N; $i++) { $s += $f($i, 1); }
    return $s;
});

bench('exception_throw_catch_100k', function () {
    $n = 0;
    for ($i = 0; $i < 100000; $i++) {
        try {
            throw new AppException('boom', $i);
        } catch (AppException $e) {
            $n += $e->getCode() & 1;
        }
    }
    return $n;
});

function thrower(int $d, int $i) {
    if ($d === 0) { throw new AppException('deep', $i); }
    return thrower($d - 1, $i);
}

bench('exception_unwind_depth8_finally_100k', function () {
    $n = 0;
    for ($i = 0; $i < 100000; $i++) {
        try {
            try { thrower(8, $i); } finally { $n++; }
        } catch (RuntimeException $e) {
            $n += $e->getCode() & 1;
        }
    }
    return $n;
});

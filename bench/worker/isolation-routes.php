<?php
// The routes of the request-isolation battery. Stateless routes must answer
// identically in worker and one-shot mode; the STATEFUL ones (marked) differ
// in exactly the way worker mode documents: statics, static properties,
// globals and boot-time objects persist across requests.
class Box { public static int $n = 0; public $v = 0; }
$GLOBALS['boot_box'] = $GLOBALS['boot_box'] ?? new Box();   // STATEFUL: created once per worker
function counter() { static $c = 0; return ++$c; }           // STATEFUL

function isolation_route(): void
{
    $path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH);
    switch ($path) {
        case '/echo':
            header('Content-Type: application/json');
            echo json_encode([
                'method' => $_SERVER['REQUEST_METHOD'], 'uri' => $_SERVER['REQUEST_URI'],
                'get' => $_GET, 'post' => $_POST, 'cookie' => $_COOKIE,
                'input' => file_get_contents('php://input'),
                'ct' => $_SERVER['CONTENT_TYPE'] ?? null, 'ua' => $_SERVER['HTTP_USER_AGENT'] ?? null,
                'script' => $_SERVER['SCRIPT_NAME'], 'qs' => $_SERVER['QUERY_STRING'] ?? null,
            ]);
            return;
        case '/headers':
            header('X-A: 1'); header('X-B: 2'); header('X-A: 3', false); http_response_code(202);
            setcookie('c', 'v', ['path' => '/']);
            echo "headers";
            return;
        case '/warn':
            echo "a"; $x = $undefined; echo "b";
            return;
        case '/exit':
            echo "before"; exit;
        case '/throw':
            throw new RuntimeException('boom');
        case '/ob':
            ob_start(); echo "inner"; $s = ob_get_clean(); echo strtoupper($s), ob_get_level();
            return;
        case '/ini':
            echo ini_get('precision'), '|'; ini_set('precision', '5'); echo ini_get('precision');
            return;
        case '/handler':
            set_error_handler(static function () { echo "H"; return true; });
            trigger_error('x', E_USER_NOTICE);
            return;
        case '/shutdown':
            register_shutdown_function(static function () { echo "[shutdown]"; });
            echo "body";
            return;
        case '/objects':
            $o = new Box(); $o->v = 7; echo spl_object_id($o) > 0 ? "obj" : "-", $o->v;
            return;
        case '/boot':  // superglobal types as the script saw them before any request
            echo $GLOBALS['boot_sg'] ?? implode(',', array_map('gettype', [$_GET, $_POST, $_COOKIE, $_FILES, $_SERVER]));
            return;
        case '/stateful':  // STATEFUL: differs by design
            Box::$n++; $GLOBALS['boot_box']->v++;
            echo "static=", Box::$n, " fn=", counter(), " boot=", $GLOBALS['boot_box']->v;
            return;
        default:
            http_response_code(404); echo "no route ", $path;
    }
}

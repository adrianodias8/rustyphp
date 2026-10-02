<?php
// The SAPI variables a cli-server router sees (oracle: php -S 8.5.7).
echo $_SERVER['SCRIPT_FILENAME'], "|", $_SERVER['SCRIPT_NAME'], "|", $_SERVER['PHP_SELF'], "|", $_SERVER['PATH_INFO'] ?? '-', "\n";
return true;

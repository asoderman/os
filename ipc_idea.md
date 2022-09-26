Request/Response architecture similar to REST.

Processes can send requests either to the kernel or to other processes then a response 
object will be placed in their file table which can be read to obtain a result or 
read/write for further communication or might not ever be read if the caller is 
indifferent to the outcome.

This also abstracts to async since the operation can make progress before the caller 
attempts to read the response object. If the caller wants to block on a result they can 
issue a read syscall.

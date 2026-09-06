// HTTP-ASM ARM64 — macOS port of HTTP-ASM64
//
// Build:  make build      (or: clang -arch arm64 -o server server.s)
// Run:    ./server        (listens on http://127.0.0.1:3926)
//
// macOS ARM64 syscall convention:
//   x16        = syscall number (BSD class)
//   x0..x5     = arguments
//   svc #0x80  = trap into kernel
//   on error   = carry flag set, x0 holds errno

.section __DATA,__data

socket_fd:      .quad   0
client_fd:      .quad   0
file_fd:        .quad   0
file_name:      .asciz  "index.html"

// BSD sockaddr_in: sin_len(1) sin_family(1) sin_port(2 BE) sin_addr(4 BE) sin_zero(8) = 16 bytes
sockaddr:
                .byte   16              // sin_len (BSD-only field)
                .byte   2               // sin_family = AF_INET
                .short  0x560f          // port 3926 in network byte order
                .long   0x0100007f      // 127.0.0.1 in network byte order
                .quad   0               // sin_zero

start_msg:      .ascii  "Listening on http://127.0.0.1:3926\n\n"
                .set    start_msg_len, . - start_msg

http200:        .ascii  "HTTP/1.1 200 OK\n"
                .ascii  "Server: HTTP-ASM-ARM64\n"
                .ascii  "Content-Type: text/html\n\n"
                .set    http200_len, . - http200

req_buff:       .space  512
res_buff:       .space  512
                .set    buff_len, 512


.section __TEXT,__text
.align 2
.global _main

_main:
    // socket(AF_INET=2, SOCK_STREAM=1, IPPROTO_TCP=6)
    mov     x0, #2
    mov     x1, #1
    mov     x2, #6
    mov     x16, #97                    // SYS_socket
    svc     #0x80
    b.cs    server_exit
    adrp    x9, socket_fd@PAGE
    str     x0, [x9, socket_fd@PAGEOFF]
    mov     x19, x0                     // x19 = listening fd (callee-saved across syscalls)

    // bind(fd, &sockaddr, 16)
    mov     x0, x19
    adrp    x1, sockaddr@PAGE
    add     x1, x1, sockaddr@PAGEOFF
    mov     x2, #16
    mov     x16, #104                   // SYS_bind
    svc     #0x80
    b.cs    server_exit

    // write(STDOUT, start_msg, start_msg_len)
    mov     x0, #1
    adrp    x1, start_msg@PAGE
    add     x1, x1, start_msg@PAGEOFF
    mov     x2, #start_msg_len
    mov     x16, #4                     // SYS_write
    svc     #0x80

    // listen(fd, backlog=8)
    mov     x0, x19
    mov     x1, #8
    mov     x16, #106                   // SYS_listen
    svc     #0x80
    b.cs    server_exit


server_accept:
    // accept(fd, NULL, NULL)
    mov     x0, x19
    mov     x1, xzr
    mov     x2, xzr
    mov     x16, #30                    // SYS_accept
    svc     #0x80
    b.cs    server_exit
    mov     x20, x0                     // x20 = client fd
    adrp    x9, client_fd@PAGE
    str     x0, [x9, client_fd@PAGEOFF]

    // read(client, req_buff, buff_len)
    mov     x0, x20
    adrp    x1, req_buff@PAGE
    add     x1, x1, req_buff@PAGEOFF
    mov     x2, #buff_len
    mov     x16, #3                     // SYS_read
    svc     #0x80
    mov     x21, x0                     // x21 = bytes read

    // write(STDOUT, req_buff, bytes_read) — log request
    mov     x0, #1
    adrp    x1, req_buff@PAGE
    add     x1, x1, req_buff@PAGEOFF
    mov     x2, x21
    mov     x16, #4
    svc     #0x80

    // open("index.html", O_RDONLY=0)
    adrp    x0, file_name@PAGE
    add     x0, x0, file_name@PAGEOFF
    mov     x1, #0
    mov     x16, #5                     // SYS_open
    svc     #0x80
    b.cs    close_client                // 404-ish: just drop the connection
    mov     x22, x0                     // x22 = file fd

    // write(client, http200, http200_len)
    mov     x0, x20
    adrp    x1, http200@PAGE
    add     x1, x1, http200@PAGEOFF
    mov     x2, #http200_len
    mov     x16, #4
    svc     #0x80


read_html:
    // read(file, res_buff, buff_len)
    mov     x0, x22
    adrp    x1, res_buff@PAGE
    add     x1, x1, res_buff@PAGEOFF
    mov     x2, #buff_len
    mov     x16, #3
    svc     #0x80
    cmp     x0, #1
    b.lt    close_file                  // EOF or error → done streaming

    // write(client, res_buff, bytes_read)
    mov     x23, x0
    mov     x0, x20
    adrp    x1, res_buff@PAGE
    add     x1, x1, res_buff@PAGEOFF
    mov     x2, x23
    mov     x16, #4
    svc     #0x80
    b       read_html


close_file:
    mov     x0, x22
    mov     x16, #6                     // SYS_close (file)
    svc     #0x80
    // fall through


close_client:
    mov     x0, x20
    mov     x16, #6                     // SYS_close (client)
    svc     #0x80
    b       server_accept


server_exit:
    mov     x0, #0
    mov     x16, #1                     // SYS_exit
    svc     #0x80

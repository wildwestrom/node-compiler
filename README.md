# Node-Based Compiler

This project was inspired by [*Kronark*](https://www.youtube.com/@Kronark).

My hope is that I can make enough simple primitives to create executable binaries and self-host the editor interface.

Right now I'm just testing out ideas. Very much a work in progress.

## Known issues

- Sometimes the nodes are arranged from top-to-bottom differently than the order the wires connect.
- Concat 2 cannot take a vec of bytes and append another byte to it.
- No ability to copy and paste nodes.
- Concat does not take an arbitrary number of wires (only 2 or 4).
- No "New" button in the menu bar to prompt the user to save or discard and make a new project.

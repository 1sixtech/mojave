// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {IMintBurnERC20} from "../BridgeGateway.sol";

contract MockWBTC is IMintBurnERC20 {
    string public constant name = "Wrapped Bitcoin";
    string public constant symbol = "WBTC";
    uint8 public constant decimals = 8;
    uint256 public totalSupply;

    mapping(address => uint256) private _balances;
    mapping(address => mapping(address => uint256)) private _allowances;

    function balanceOf(address account) external view returns (uint256) {
        return _balances[account];
    }

    function allowance(
        address owner,
        address spender
    ) external view returns (uint256) {
        return _allowances[owner][spender];
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        _allowances[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(_balances[msg.sender] >= amount, "insufficient balance");
        _balances[msg.sender] -= amount;
        _balances[to] += amount;
        emit Transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(
        address from,
        address to,
        uint256 amount
    ) external returns (bool) {
        require(_balances[from] >= amount, "insufficient balance");
        require(
            _allowances[from][msg.sender] >= amount,
            "insufficient allowance"
        );
        _balances[from] -= amount;
        _balances[to] += amount;
        if (_allowances[from][msg.sender] != type(uint256).max) {
            _allowances[from][msg.sender] -= amount;
        }
        emit Transfer(from, to, amount);
        return true;
    }

    function mint(address to, uint256 amount) external {
        _balances[to] += amount;
        totalSupply += amount;
        emit Transfer(address(0), to, amount);
    }

    function burn(uint256 amount) external {
        require(_balances[msg.sender] >= amount, "insufficient balance");
        _balances[msg.sender] -= amount;
        totalSupply -= amount;
        emit Transfer(msg.sender, address(0), amount);
    }
}

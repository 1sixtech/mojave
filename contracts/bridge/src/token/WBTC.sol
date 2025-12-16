// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";

/**
 * @title WBTC - Wrapped Bitcoin on Mojave L2
 * @notice ERC20 token representing Bitcoin with 8 decimals (1:1 with satoshis)
 * @dev Only authorized minters (bridge contracts) can mint/burn tokens
 *
 * Architecture:
 * - Uses AccessControl for role-based permissions
 * - MINTER_ROLE: Can mint new tokens (granted to BridgeGateway for deposits)
 * - BURNER_ROLE: Can burn tokens from any address (granted to BridgeGateway for withdrawals)
 * - DEFAULT_ADMIN_ROLE: Can manage roles
 */
contract WBTC is ERC20, AccessControl {
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");

    /**
     * @notice Emitted when tokens are minted
     * @param to Recipient address
     * @param amount Amount minted (in satoshis with 8 decimals)
     * @param minter Address that triggered the mint
     */
    event Minted(address indexed to, uint256 amount, address indexed minter);

    /**
     * @notice Emitted when tokens are burned
     * @param from Address tokens were burned from
     * @param amount Amount burned (in satoshis with 8 decimals)
     * @param burner Address that triggered the burn
     */
    event Burned(address indexed from, uint256 amount, address indexed burner);

    /**
     * @notice Constructor
     * @param admin Initial admin address (can grant/revoke roles)
     */
    constructor(address admin) ERC20("Wrapped Bitcoin", "WBTC") {
        require(admin != address(0), "WBTC: zero admin");

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MINTER_ROLE, admin);
        _grantRole(BURNER_ROLE, admin);
    }

    /**
     * @notice Returns 8 decimals to match Bitcoin satoshis
     * @return Number of decimals (8)
     */
    function decimals() public pure override returns (uint8) {
        return 8;
    }

    /**
     * @notice Mint tokens (only MINTER_ROLE)
     * @param to Recipient address
     * @param amount Amount to mint in satoshis (with 8 decimals)
     */
    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) {
        require(to != address(0), "WBTC: mint to zero");
        require(amount > 0, "WBTC: zero amount");

        _mint(to, amount);
        emit Minted(to, amount, msg.sender);
    }

    /**
     * @notice Burn tokens from caller (public, anyone can burn their own tokens)
     * @param amount Amount to burn in satoshis (with 8 decimals)
     */
    function burn(uint256 amount) public {
        require(amount > 0, "WBTC: zero amount");

        _burn(msg.sender, amount);
        emit Burned(msg.sender, amount, msg.sender);
    }

    /**
     * @notice Burn tokens from specific address
     * @param from Address to burn from
     * @param amount Amount to burn in satoshis (with 8 decimals)
     * @dev If caller has BURNER_ROLE: burns directly without allowance
     * @dev Otherwise: requires allowance (standard ERC20 burnFrom behavior)
     */
    function burnFrom(address from, uint256 amount) public {
        require(from != address(0), "WBTC: burn from zero");
        require(amount > 0, "WBTC: zero amount");

        // If caller has BURNER_ROLE, skip allowance check
        // This allows BridgeGateway to burn locked tokens during withdrawals
        if (hasRole(BURNER_ROLE, msg.sender)) {
            _burn(from, amount);
        } else {
            // Standard ERC20 burnFrom: requires allowance
            _spendAllowance(from, msg.sender, amount);
            _burn(from, amount);
        }

        emit Burned(from, amount, msg.sender);
    }

    /**
     * @notice Grant minter role to bridge contract
     * @param bridge Bridge contract address
     */
    function addMinter(address bridge) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(bridge != address(0), "WBTC: zero bridge");
        grantRole(MINTER_ROLE, bridge);
    }

    /**
     * @notice Grant burner role to bridge contract
     * @param bridge Bridge contract address
     */
    function addBurner(address bridge) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(bridge != address(0), "WBTC: zero bridge");
        grantRole(BURNER_ROLE, bridge);
    }

    /**
     * @notice Revoke minter role
     * @param account Address to revoke from
     */
    function removeMinter(
        address account
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        revokeRole(MINTER_ROLE, account);
    }

    /**
     * @notice Revoke burner role
     * @param account Address to revoke from
     */
    function removeBurner(
        address account
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        revokeRole(BURNER_ROLE, account);
    }

    /**
     * @notice Check if address has minter role
     * @param account Address to check
     * @return True if address is a minter
     */
    function isMinter(address account) external view returns (bool) {
        return hasRole(MINTER_ROLE, account);
    }

    /**
     * @notice Check if address has burner role
     * @param account Address to check
     * @return True if address is a burner
     */
    function isBurner(address account) external view returns (bool) {
        return hasRole(BURNER_ROLE, account);
    }
}

import { InMemoryPlacesRepository } from "../../src/storage/InMemoryPlacesRepository.js";
import { sharedRepositoryContractTests } from "../../src/storage/repositoryContractTests.js";

sharedRepositoryContractTests(() => new InMemoryPlacesRepository());
